// HolodeckShader.swift
// CrystalKit
//
// Metal shader source for the Liquid Glass composite pass.
// Extracted from Swiftlight's LiquidGlassRenderer — identical shader code.
//
// The shader implements:
// 1. SDF shape evaluation (rect, ellipse, polygon, star) → dome-like normal field
// 2. Refraction via UV offset (Snell's law, IOR ~1.5)
// 3. Chromatic aberration at edges (7-band spectral sampling)
// 4. Directional rim light with configurable angle
// 5. Fresnel edge reflection (Schlick's approximation)
// 6. Frost shimmer, tint, and specular highlights

// swiftlint:disable file_length

enum HolodeckShaderSource {
    // swiftformat:disable all
    static let source = """
    #include <metal_stdlib>
    using namespace metal;

    struct HolodeckViewportUniforms {
        float4x4 viewProjection;
    };

    struct HolodeckQuadInstance {
        float2 position;
        float2 size;
        float rotation;
        float opacity;
        float scale;
        float _pad;
        float2 captureHalfExtent;  // axis-aligned capture region half-size (canvas units)
        float2 captureOffset;      // sub-pixel offset: actual capture center - node center (canvas units)
    };

    struct HolodeckCompositeUniforms {
        float4 tintColor;
        float tintOpacity;
        float desaturation;
        float specularIntensity;
        uint glassVariant;       // 0 = regular, 1 = clear
        float4 cornerRadii;     // TL, TR, BR, BL
        float2 size;
        float2 blurPadding;     // Normalized padding (padPixels / textureSize) per axis
        float refractionStrength;   // UV offset multiplier
        float dispersion;           // Chromatic aberration multiplier
        float depthScale;           // Magnification: 1.0 = no zoom, >1 = magnify
        float splayStrength;        // Radial barrel distortion amount (0+)
        float lightAngle;           // Light direction in radians
        float lightIntensity;       // Light effect strength (0-1)
        float lightBanding;         // Gradient falloff: 0=sharp cutoff, 1=soft feathered fade
        uint shapeType;             // 0=rect, 1=ellipse, 2=polygon, 3=star
        uint sides;                 // Polygon/star side count
        float innerRadius;          // Star inner radius ratio
        float outerRadius;          // Star outer radius ratio
        float polygonBorderRadius;  // Polygon/star outer corner radius
        float starInnerBorderRadius;// Star inner corner radius
        float cornerSmoothing;      // Rectangle corner smoothing
        float4 maskConfig;          // xy mask padding, z useSDFTexture, w reserved
        float edgeWidth;            // 0-1: how far refraction extends inward from edges
        float frost;                // 0-1: frost intensity (drives blur + crystal refraction)
        float rotation;             // Node rotation in radians (for local→screen UV transform)
        float canvasZoom;           // Current canvas zoom level
        float2 cursorWorldPos;      // Cursor position in world/canvas coords. (-inf,-inf)=inactive.
        float cursorActive;         // 1.0 = cursor fluid mode, 0.0 = fixed angular mode
        uint resonanceEnabled;      // 1 = adaptive bg-luminance tinting, 0 = static tint
        float2 viewportCenter;      // Shape center in [0,1] viewport UV (for edge parallax)
        float viewportScale;        // Shape size / viewport size (for parallax scaling)
        float3 smoothedResonanceTint; // CPU anchor tint for temporal smoothing
        float resonanceBlendFactor;  // 0-1: how much fresh probe overrides anchor (0.08 typical)
        uint _pad3; // struct alignment padding
        uint luminanceEnabled;          // 1 = inner light/shadow from backdrop luminance
        uint brillianceCount;           // Number of active light sources (0-4)
        float2 brillianceSource0;       // World position of light source 0
        float2 brillianceSource1;       // World position of light source 1
        float2 brillianceSource2;       // World position of light source 2
        float2 brillianceSource3;       // World position of light source 3
        float brillianceMargin;         // Quad expansion for flare bleed (canvas pts)
        float3 brillianceTint0;         // Per-light adaptive tint color (white if unused)
        float3 brillianceTint1;
        float3 brillianceTint2;
        float3 brillianceTint3;
        float2 cropUVOffset;             // Top-left UV of glass region in full backdrop texture
        float2 cropUVScale;              // UV span of glass region in full backdrop texture
        float glowIntensity;             // 0 = off, 0-1+ = glow strength
        uint glowBlendMode;              // 0 = screen, 1 = additive, 2 = soft light
    };

    struct GlassVertexOut {
        float4 position [[position]];
        float2 uv;              // [0, 1] within glass quad (node-local)
        float2 screenUV;        // [0, 1] within axis-aligned capture region (screen-space)
        float2 worldPos;        // pixel position in canvas/world coordinates
        float opacity;
    };

    // Rounded rect SDF for glass shape masking
    float sdGlassRect(float2 p, float2 b, float4 r) {
        float2 rr;
        rr.x = (p.x > 0.0) ? ((p.y > 0.0) ? r.z : r.y) : ((p.y > 0.0) ? r.w : r.x);
        rr.y = rr.x;
        float2 q = abs(p) - b + rr;
        return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr.x;
    }

    // Superellipse variant for corner smoothing on rounded rectangles.
    float sdSuperellipseRect(float2 p, float2 b, float4 radii, float smoothing) {
        float dRound = sdGlassRect(p, b, radii);
        if (smoothing <= 0.0) return dRound;

        float n = 2.0 + smoothing * 3.0;
        float2 ap = abs(p);
        float2 halfExt = b;
        float2 normalized = ap / max(halfExt, float2(0.001));
        float se = pow(pow(normalized.x, n) + pow(normalized.y, n), 1.0 / n) - 1.0;

        float avgRadius = (radii.x + radii.y + radii.z + radii.w) * 0.25;
        float blend = clamp(smoothing, 0.0, 1.0);
        float cornerDist = min(halfExt.x - ap.x, halfExt.y - ap.y);
        float cornerBlend = 1.0 - smoothstep(0.0, avgRadius * 2.0, cornerDist);

        return mix(dRound, se * max(halfExt.x, halfExt.y) * 0.5, blend * cornerBlend);
    }

    // Signed distance to an ellipse.
    float sdEllipse(float2 p, float2 ab) {
        float2 pa = abs(p);
        float2 r = ab;
        if (abs(r.x - r.y) < 0.001) return length(pa) - r.x;

        float2 q = pa;
        if (r.x < r.y) {
            q = q.yx;
            r = r.yx;
        }
        float a2 = r.x * r.x;
        float b2 = r.y * r.y;
        float t = 0.7853981633;

        for (int i = 0; i < 3; i++) {
            float ct = cos(t);
            float st = sin(t);
            float2 e = float2(a2 - b2, 0.0) * float2(ct * ct * ct, st * st * st) / r;
            float2 w = q - e;
            float2 g = float2(r.x * ct, r.y * st) - e;
            float gLen = length(g);
            if (gLen > 0.0001) {
                t += atan2(cross(float3(w, 0), float3(g, 0)).z, dot(w, g)) / gLen;
            }
        }

        float2 closest = float2(r.x * cos(t), r.y * sin(t));
        float dist = length(pa - closest);
        float inside = (pa.x * pa.x) / a2 + (pa.y * pa.y) / b2;
        return (inside < 1.0) ? -dist : dist;
    }

    // Signed distance to a regular polygon.
    float sdPolygon(float2 p, uint sides, float radius, float borderRadius) {
        float n = float(max(sides, 3u));
        float an = 3.14159265 / n; // half-angle per sector
        // Using atan2(x, y) intentionally orients even-sided polygons with a top vertex.
        float sector = 2.0 * an;
        float wrapped = atan2(p.x, p.y) + an;
        wrapped -= sector * floor(wrapped / sector); // positive modulo
        float bn = wrapped - an;
        float d = length(p) * cos(bn) - radius * cos(an);
        if (borderRadius > 0.0) d -= borderRadius;
        return d;
    }

    // Signed distance to an N-pointed star with straight edges.
    float sdStar(float2 p, uint points, float innerR, float outerR,
                 float borderRadius, float innerBorderRadius) {
        float n = float(max(points, 3u));
        float an = 3.14159265 / n;

        float angle = atan2(p.x, p.y);
        float sector = fmod(angle + 3.14159265, 2.0 * an);

        float2 outerVtx = float2(sin(0.0), cos(0.0)) * outerR;
        float2 innerVtx = float2(sin(an), cos(an)) * innerR;

        float localAngle = sector;
        float r = length(p);
        float2 localP = float2(sin(localAngle), cos(localAngle)) * r;

        float d;
        if (sector < an) {
            float2 edge = innerVtx - outerVtx;
            float2 toP = localP - outerVtx;
            float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
            float2 closest = outerVtx + edge * t;
            d = length(localP - closest);
        } else {
            float2 outerVtx2 = float2(sin(2.0 * an), cos(2.0 * an)) * outerR;
            float2 edge = outerVtx2 - innerVtx;
            float2 toP = localP - innerVtx;
            float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
            float2 closest = innerVtx + edge * t;
            d = length(localP - closest);
        }

        float2 edgeA, edgeB;
        if (sector < an) {
            edgeA = outerVtx; edgeB = innerVtx;
        } else {
            edgeA = innerVtx;
            edgeB = float2(sin(2.0 * an), cos(2.0 * an)) * outerR;
        }
        float2 edgeDir = edgeB - edgeA;
        float2 toP = localP - edgeA;
        float cross = edgeDir.x * toP.y - edgeDir.y * toP.x;
        d = (cross > 0.0) ? d : -d;

        if (borderRadius > 0.0 || innerBorderRadius > 0.0) {
            float tNorm = sector / an;
            float tipBlend = abs(tNorm - 1.0);
            float br = mix(innerBorderRadius, borderRadius, tipBlend);
            if (br > 0.0) {
                d += br;
                d = max(d, 0.0);
                d -= br;
            }
        }
        return d;
    }

    // Smooth signed distance to a star — gradient-safe version.
    float sdStarSmooth(float2 p, uint points, float innerR, float outerR) {
        float n = float(max(points, 3u));
        float an = 3.14159265 / n;

        float k = 16.0;
        float sumExp = 0.0;

        for (uint i = 0; i < points; i++) {
            float baseAngle = 2.0 * 3.14159265 * float(i) / n - 3.14159265 * 0.5;
            float nextBase = 2.0 * 3.14159265 * float((i + 1) % points) / n - 3.14159265 * 0.5;

            float2 outerPt = float2(cos(baseAngle), sin(baseAngle)) * outerR;
            float innerAngle = baseAngle + an;
            float2 innerPt = float2(cos(innerAngle), sin(innerAngle)) * innerR;
            float2 nextOuterPt = float2(cos(nextBase), sin(nextBase)) * outerR;

            {
                float2 edge = innerPt - outerPt;
                float2 toP = p - outerPt;
                float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
                float2 closest = outerPt + edge * t;
                float dist = length(p - closest);
                sumExp += exp(-k * dist);
            }
            {
                float2 edge = nextOuterPt - innerPt;
                float2 toP = p - innerPt;
                float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
                float2 closest = innerPt + edge * t;
                float dist = length(p - closest);
                sumExp += exp(-k * dist);
            }
        }

        float smoothDist = -log(max(sumExp, 1e-20)) / k;

        float winding = 0.0;
        for (uint i = 0; i < points; i++) {
            float baseAngle = 2.0 * 3.14159265 * float(i) / n - 3.14159265 * 0.5;
            float nextBase = 2.0 * 3.14159265 * float((i + 1) % points) / n - 3.14159265 * 0.5;
            float2 outerPt = float2(cos(baseAngle), sin(baseAngle)) * outerR;
            float innerAngle = baseAngle + an;
            float2 innerPt = float2(cos(innerAngle), sin(innerAngle)) * innerR;
            float2 nextOuterPt = float2(cos(nextBase), sin(nextBase)) * outerR;

            float2 e1 = innerPt - outerPt;
            float2 d1 = p - outerPt;
            winding += e1.x * d1.y - e1.y * d1.x;
            float2 e2 = nextOuterPt - innerPt;
            float2 d2 = p - innerPt;
            winding += e2.x * d2.y - e2.y * d2.x;
        }

        return (winding > 0.0) ? smoothDist : -smoothDist;
    }

    float sdGlassShape(float2 localPos, float2 halfSize, float4 radii, constant HolodeckCompositeUniforms &glass) {
        float minHalf = min(halfSize.x, halfSize.y);
        switch (glass.shapeType) {
            case 1:
                return sdEllipse(localPos, halfSize);
            case 2: {
                float2 scale = halfSize / max(minHalf, 1e-4);
                float2 scaledPos = localPos / scale;
                return sdPolygon(scaledPos, glass.sides, minHalf, glass.polygonBorderRadius);
            }
            case 3: {
                float2 scale = halfSize / max(minHalf, 1e-4);
                float2 scaledPos = localPos / scale;
                float outerR = minHalf * glass.outerRadius;
                float innerR = outerR * glass.innerRadius;
                return sdStar(
                    scaledPos,
                    glass.sides,
                    innerR,
                    outerR,
                    glass.polygonBorderRadius,
                    glass.starInnerBorderRadius
                );
            }
            default:
                return sdSuperellipseRect(localPos, halfSize, radii, glass.cornerSmoothing);
        }
    }

    float2 safeNormalize(float2 v) {
        float len = length(v);
        if (len < 1e-5) return float2(0.0, -1.0);
        return v / len;
    }

    /// Evaluate the shape SDF at an arbitrary local position, using the SDF
    /// texture when available (custom vector paths) or the analytic SDF otherwise.
    /// Returns signed distance in canvas points (negative inside, positive outside).
    float sampleShapeSDF(float2 localPos, float2 halfSize, float4 radii,
                         constant HolodeckCompositeUniforms &glass,
                         texture2d<float> maskTex, sampler s,
                         bool useSDFTexture, float2 sdfMaskPadding,
                         float2 sdfMaskRange, float sdfTexelToPoint, float2 quadSize) {
        if (useSDFTexture) {
            // Convert localPos → UV → SDF texture UV.
            // Use glass.size (not quadSize) because the SDF texture covers the
            // shape at its original size — quadSize includes brilliance margin
            // which would compress the UV and cause ghosts to sample the padded
            // border region (producing rectangular shapes for text/paths).
            float2 uv = localPos / glass.size + 0.5;
            float2 sdfUV = sdfMaskPadding + uv * sdfMaskRange;
            // If the query falls outside the SDF texture, return a large
            // positive distance so the Gaussian kills the contribution
            // instead of clamping to the border (which reads ~0 and fogs).
            if (sdfUV.x < 0.0 || sdfUV.x > 1.0 || sdfUV.y < 0.0 || sdfUV.y > 1.0) {
                return 1e4;
            }
            float sdfValue = maskTex.sample(s, sdfUV).r;
            return sdfValue * sdfTexelToPoint;
        }
        return sdGlassShape(localPos, halfSize, radii, glass);
    }

    /// Shape-conforming ghost orb.
    ///
    /// Creates a miniature copy of the glass shape at the ghost position.
    /// The pixel offset from ghost center is rescaled so that `radius` maps to
    /// the glass boundary, then fed into the shape SDF. Produces ghosts that are
    /// rectangular for rectangles, star-shaped for stars, path-shaped for vector
    /// paths, etc.
    float shapeGhost(float2 pixelPos, float2 ghostCenter, float radius,
                     float2 halfSize, float4 radii, float ghostDelta, float mh,
                     constant HolodeckCompositeUniforms &glass,
                     texture2d<float> maskTex, sampler s,
                     bool useSDFTexture, float2 sdfMaskPadding,
                     float2 sdfMaskRange, float sdfTexelToPoint, float2 quadSize) {
        // Rescale pixel offset so radius maps to the full shape size
        float scale = mh / max(radius, 1e-4);
        float2 sdfPos = (pixelPos - ghostCenter) * scale;

        float d = sampleShapeSDF(sdfPos, halfSize, radii, glass, maskTex, s,
                                  useSDFTexture, sdfMaskPadding, sdfMaskRange,
                                  sdfTexelToPoint, quadSize);

        // Filled ghost: solid inside the shape (d < 0), Gaussian fade outside.
        // Frost widens the falloff so ghosts blur out like the frosted backdrop.
        // SDF textures cap the total falloff to prevent fog across the bounding box.
        float frostBlur = 1.0 + clamp(glass.frost, 0.0, 1.0) * 6.0;
        float baseFalloff = useSDFTexture ? 0.05 : 0.15;
        float maxFalloff = useSDFTexture ? 0.12 : 1e6;
        float falloff = min(mh * baseFalloff * frostBlur, mh * maxFalloff);
        float dOut = max(d, 0.0);
        return exp(-(dOut * dOut) / (falloff * falloff));
    }

    /// Shape-conforming filled ring ghost: solid inside, soft fade outside.
    float shapeRing(float2 pixelPos, float2 ghostCenter, float radius, float ringWidth,
                    float2 halfSize, float4 radii, float ghostDelta, float mh,
                    constant HolodeckCompositeUniforms &glass,
                    texture2d<float> maskTex, sampler s,
                    bool useSDFTexture, float2 sdfMaskPadding,
                    float2 sdfMaskRange, float sdfTexelToPoint, float2 quadSize) {
        float scale = mh / max(radius, 1e-4);
        float2 sdfPos = (pixelPos - ghostCenter) * scale;

        float d = sampleShapeSDF(sdfPos, halfSize, radii, glass, maskTex, s,
                                  useSDFTexture, sdfMaskPadding, sdfMaskRange,
                                  sdfTexelToPoint, quadSize);

        float frostBlur = 1.0 + clamp(glass.frost, 0.0, 1.0) * 6.0;
        float baseRingFalloff = useSDFTexture ? 0.02 : 0.06;
        float maxRingFalloff = useSDFTexture ? 0.05 : 1e6;
        float ringFalloff = min(mh * baseRingFalloff * frostBlur, mh * maxRingFalloff);
        float dOut = max(d, 0.0);
        return exp(-(dOut * dOut) / (ringFalloff * ringFalloff));
    }

    /// Gaussian displacement falloff from edge (depth=0) to center (depth=1).
    /// σ scales with edgeExp — higher exponent = tighter bell.
    float refractionFalloff(float depth, float edgeExp) {
        float d = clamp(depth, 0.0, 1.0);
        float sigma = mix(0.5, 0.08, clamp((edgeExp - 2.0) / 4.0, 0.0, 1.0));
        return exp(-(d * d) / (sigma * sigma));
    }

    // Precomputed spectral weights for 7-band chromatic sampling.
    // t = band / 6.0; w = float3(exp(-8*t*t), exp(-8*(t-0.5)^2), exp(-8*(t-1)^2))
    constexpr constant float3 spectralW[7] = {
        float3(1.000000, 0.135335, 0.000335),
        float3(0.795845, 0.245960, 0.001140),
        float3(0.401154, 0.527292, 0.004575),
        float3(0.135335, 1.000000, 0.018316),
        float3(0.029590, 0.527292, 0.082085),
        float3(0.004087, 0.245960, 0.401154),
        float3(0.000335, 0.135335, 1.000000),
    };

    float glassHeight(float interiorDist, float rimDist) {
        return smoothstep(0.0, max(rimDist, 1e-4), interiorDist);
    }

    float evalInteriorDist(float2 pos, float2 halfSize, float4 radii,
                           constant HolodeckCompositeUniforms &glass) {
        float d = sdGlassShape(pos, halfSize, radii, glass);
        float dist = max(-d, 0.0);
        if (glass.shapeType == 1) {
            float2 e = max(halfSize, float2(1e-4));
            float2 pe = pos / e;
            float minH = min(halfSize.x, halfSize.y);
            dist = max(-(dot(pe, pe) - 1.0), 0.0) * minH * 0.5;
        }
        return dist;
    }

    float2 glassInwardNormal(float2 localPos, float2 halfSize, float4 radii, constant HolodeckCompositeUniforms &glass) {
        if (glass.shapeType == 1) {
            float2 e = max(halfSize, float2(1e-4));
            float2 outward = float2(localPos.x / (e.x * e.x), localPos.y / (e.y * e.y));
            return -safeNormalize(outward);
        }

        float2 norm = localPos / max(halfSize, float2(1e-4));
        return -safeNormalize(norm);
    }

    vertex GlassVertexOut vertex_glass_quad(
        uint vid [[vertex_id]],
        constant HolodeckViewportUniforms &viewport [[buffer(0)]],
        constant HolodeckQuadInstance &inst [[buffer(1)]]
    ) {
        constexpr float2 quadVerts[6] = {
            float2(0, 0), float2(1, 0), float2(0, 1),
            float2(1, 0), float2(1, 1), float2(0, 1)
        };

        float2 uv = quadVerts[vid];
        float2 local = (uv - 0.5) * inst.size * inst.scale;

        float cosR = cos(inst.rotation);
        float sinR = sin(inst.rotation);
        float2 rotated = float2(
            local.x * cosR - local.y * sinR,
            local.x * sinR + local.y * cosR
        );

        float2 world = rotated + inst.position;
        float4 clipPos = viewport.viewProjection * float4(world, 0.0, 1.0);

        float2 screenUV = (rotated - inst.captureOffset) / (inst.captureHalfExtent * 2.0) + 0.5;

        GlassVertexOut out;
        out.position = clipPos;
        out.uv = uv;
        out.screenUV = screenUV;
        out.worldPos = world;
        out.opacity = inst.opacity;
        return out;
    }

    // ── Rim Light Pass ──────────────────────────────────────────────────
    // Renders ONLY the rim light to an intermediate texture so it can be
    // sampled with refraction UV offsets in the composite pass, making
    // the light bend with the glass.
    fragment float4 fragment_rim_light(
        GlassVertexOut in [[stage_in]],
        texture2d<float> maskTex [[texture(0)]],
        constant HolodeckCompositeUniforms &glass [[buffer(0)]]
    ) {
        float2 localPos = (in.uv - 0.5) * glass.size;
        float2 halfSize = glass.size * 0.5;
        float4 radii = min(glass.cornerRadii, float4(min(halfSize.x, halfSize.y)));
        float d = sdGlassShape(localPos, halfSize, radii, glass);

        float2 dpdx_lp = dfdx(localPos);
        float2 dpdy_lp = dfdy(localPos);
        float pixelSize = length(float2(length(dpdx_lp), length(dpdy_lp))) * 0.7071;
        float aa = max(pixelSize, 0.75);
        float shapeMask = 1.0 - smoothstep(-aa, aa, d);

        float interiorDistance = max(-d, 0.0);

        if (glass.shapeType == 1) {
            float2 eSmooth = max(halfSize, float2(1e-4));
            float2 peSmooth = localPos / eSmooth;
            float minHalfE = min(halfSize.x, halfSize.y);
            interiorDistance = max(-(dot(peSmooth, peSmooth) - 1.0), 0.0) * minHalfE * 0.5;
        }

        float falloffFrac = mix(0.01, 0.60, glass.edgeWidth);
        float falloffPx = max(6.0, min(halfSize.x, halfSize.y) * falloffFrac);
        float edgeProximity = 1.0 - smoothstep(0.0, falloffPx, interiorDistance);

        bool hasSdfTexture = glass.maskConfig.z > 0.5;
        bool useSDFTexture = hasSdfTexture && (glass.shapeType != 3);
        if (useSDFTexture) {
            constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);
            float2 sdfMaskPadding = clamp(glass.maskConfig.xy, 0.0, 0.49);
            float2 sdfMaskRange = max(1.0 - 2.0 * sdfMaskPadding, float2(1e-4));
            float2 sdfUV = clamp(sdfMaskPadding + in.uv * sdfMaskRange, 0.0, 1.0);
            float2 sdfTexSize = float2(float(maskTex.get_width()), float(maskTex.get_height()));
            float2 texelsPerPt = (sdfTexSize * sdfMaskRange) / max(glass.size, float2(1.0));
            float sdfTexelToPoint = 1.0 / max(0.5 * (texelsPerPt.x + texelsPerPt.y), 1e-4);

            float4 sdfSample = maskTex.sample(s, sdfUV);
            float sdfValue = sdfSample.r;
            // Use UV-space derivatives for stable AA — measures how many
            // texels each pixel covers, giving a consistent edge softness
            // regardless of SDF gradient noise at texel boundaries.
            float2 sdfUVdx = dfdx(sdfUV);
            float2 sdfUVdy = dfdy(sdfUV);
            float texelFootprint = length(float2(length(sdfUVdx), length(sdfUVdy)))
                                 * max(sdfTexSize.x, sdfTexSize.y) * 0.7071;
            float sdfAA = max(texelFootprint, 0.75);
            shapeMask = 1.0 - smoothstep(-sdfAA, sdfAA, sdfValue);

            float sdfPoints = sdfValue * sdfTexelToPoint;
            float sdfInterior = max(-sdfPoints, 0.0);
            float heightVal = sdfSample.a;
            float heightInterior = heightVal * sdfTexelToPoint;
            float edgeBlend = smoothstep(3.0, 6.0, sdfInterior);
            interiorDistance = mix(sdfInterior, heightInterior, edgeBlend);
            edgeProximity = 1.0 - smoothstep(0.0, falloffPx, interiorDistance);
        }

        if (shapeMask < 0.001) {
            return float4(0.0);
        }

        float minHalf = min(halfSize.x, halfSize.y);
        float maxInteriorDist = minHalf * 0.5;
        float pixelAngle = atan2(localPos.x, -localPos.y);

        // Surface normal for Fresnel
        float2 surfaceNormal;
        if (glass.shapeType == 1) {
            float2 e = max(halfSize, float2(1e-4));
            float2 outward = float2(localPos.x / (e.x * e.x), localPos.y / (e.y * e.y));
            surfaceNormal = safeNormalize(outward);
        } else if (glass.shapeType == 2) {
            // Nearest-edge normal — matches the composite pass so rim light
            // and refraction agree on surface orientation. The generic SDF
            // finite-difference gradient creates medial-axis artifacts.
            float polyMinHalf = min(halfSize.x, halfSize.y);
            float2 polyScale = halfSize / max(polyMinHalf, 1e-4);
            float2 polyPos = localPos / polyScale;

            uint pn = max(glass.sides, 3u);
            float fn = float(pn);
            float pan = 3.14159265 / fn;

            float bestDist = 1e10;
            float secondBestDist = 1e10;
            float2 bestNormal = float2(0.0, -1.0);
            for (uint i = 0; i < pn; i++) {
                float a0 = float(i) * 2.0 * pan;
                float a1 = float(i + 1) * 2.0 * pan;
                float2 v0 = polyMinHalf * float2(sin(a0), cos(a0));
                float2 v1 = polyMinHalf * float2(sin(a1), cos(a1));
                float2 edgeMid = 0.5 * (v0 + v1);
                float2 eNorm = safeNormalize(edgeMid);
                float eDist = abs(dot(polyPos - v0, eNorm));
                if (eDist < bestDist) {
                    secondBestDist = bestDist;
                    bestDist = eDist;
                    bestNormal = eNorm;
                } else if (eDist < secondBestDist) {
                    secondBestDist = eDist;
                }
            }

            // Blend toward centroid near medial axis for smooth transitions
            float edgeRatio = bestDist / max(secondBestDist, 1e-4);
            float medialBlend = smoothstep(0.7, 0.95, edgeRatio);
            float2 centroidDir = safeNormalize(localPos);
            bestNormal = safeNormalize(bestNormal * polyScale);
            surfaceNormal = normalize(mix(bestNormal, centroidDir, medialBlend));
        } else if (glass.shapeType == 3) {
            float starMinHalf = min(halfSize.x, halfSize.y);
            float2 starScale = halfSize / max(starMinHalf, 1e-4);
            float2 starPos = localPos / starScale;
            float outerR = starMinHalf * glass.outerRadius;
            float innerR = outerR * glass.innerRadius;
            float midR = (outerR + innerR) * 0.5;
            float ampR = (outerR - innerR) * 0.5;
            float n = float(max(glass.sides, 3u));
            float delta = max(length(dfdx(starPos)), 0.5) * 2.0;
            float2 sp1 = starPos + float2(delta, 0.0);
            float2 sp2 = starPos + float2(-delta, 0.0);
            float2 sp3 = starPos + float2(0.0, -delta);
            float2 sp4 = starPos + float2(0.0, delta);
            float wr1 = length(sp1) / max(midR + ampR * cos(n * atan2(sp1.x, sp1.y)), 1e-4);
            float wr2 = length(sp2) / max(midR + ampR * cos(n * atan2(sp2.x, sp2.y)), 1e-4);
            float wr3 = length(sp3) / max(midR + ampR * cos(n * atan2(sp3.x, sp3.y)), 1e-4);
            float wr4 = length(sp4) / max(midR + ampR * cos(n * atan2(sp4.x, sp4.y)), 1e-4);
            float2 warpGrad = float2(wr1 - wr2, wr4 - wr3);
            surfaceNormal = safeNormalize(warpGrad);
        } else {
            float delta = max(length(dfdx(localPos)), 0.5) * 2.0;
            float dR = sdGlassShape(localPos + float2( delta, 0.0), halfSize, radii, glass);
            float dL = sdGlassShape(localPos + float2(-delta, 0.0), halfSize, radii, glass);
            float dU = sdGlassShape(localPos + float2(0.0, -delta), halfSize, radii, glass);
            float dD = sdGlassShape(localPos + float2(0.0,  delta), halfSize, radii, glass);
            float2 sdfGrad = float2(dR - dL, dD - dU);
            surfaceNormal = safeNormalize(sdfGrad);
        }

        // Rotation: transform world-space directions into local (unrotated)
        // space so they match the surface normals computed from the SDF.
        float cosRfwd = cos(glass.rotation);  // forward rotation (local → world)
        float sinRfwd = sin(glass.rotation);
        float cosR = cosRfwd;   // cos(-x) == cos(x)
        float sinR = -sinRfwd;  // sin(-x) == -sin(x)

        // Node center in world space (rotation-aware)
        float2 rotatedLocal = float2(
            localPos.x * cosRfwd - localPos.y * sinRfwd,
            localPos.x * sinRfwd + localPos.y * cosRfwd
        );
        float2 nodeCenter = in.worldPos - rotatedLocal;

        // Light direction in local space
        float2 lightDirWorld;
        if (glass.cursorActive > 0.5) {
            float2 towardCursor = glass.cursorWorldPos - nodeCenter;
            float tcLen = length(towardCursor);
            lightDirWorld = tcLen > 0.001 ? towardCursor / tcLen : float2(0.0, -1.0);
        } else {
            lightDirWorld = float2(sin(glass.lightAngle), -cos(glass.lightAngle));
        }

        float2 lightDir = float2(
            lightDirWorld.x * cosR - lightDirWorld.y * sinR,
            lightDirWorld.x * sinR + lightDirWorld.y * cosR
        );

        float2 litEdgeDir = float2(-lightDir.y, lightDir.x);
        float2 absEdge = abs(litEdgeDir);
        float litEdgeLen;
        if (glass.shapeType == 1) {
            litEdgeLen = length(halfSize * absEdge);
        } else {
            litEdgeLen = dot(halfSize, absEdge);
        }
        litEdgeLen = max(litEdgeLen, 1.0);

        float rimWidth = max(litEdgeLen * 0.10, 4.0);
        float gleamWidth = max(litEdgeLen * 0.40, 10.0);

        float normalDotLight = dot(surfaceNormal, lightDir);
        float fresnelGraze = 1.0 - abs(normalDotLight);
        float fg2 = fresnelGraze * fresnelGraze;

        float fRimWidth = rimWidth * mix(0.3, 1.5, fg2);
        float fGleamWidth = gleamWidth * mix(0.4, 1.3, fg2);

        float edgeProximityNorm = clamp(interiorDistance / max(maxInteriorDist, 1.0), 0.0, 1.0);
        float fresnelEdgeMask = 1.0 - smoothstep(0.0, 0.35, edgeProximityNorm);
        float fresnelBoost = mix(0.5, 1.2, fg2 * fresnelEdgeMask);

        float variantLightScale = (glass.glassVariant == 1) ? 0.70 : 1.0;

        // Rim lighting computation (same as composite was doing analytically)
        float si = glass.specularIntensity;
        float li = glass.lightIntensity;
        float lb = glass.lightBanding;
        // Hard outer edge, soft inner fade: 1-pixel AA ramp at the boundary
        // so the light cuts off crisply on the outside but feathers inward
        // via the Gaussian falloff (sRimFalloff / sGleamFalloff).
        float edgeFade = smoothstep(0.0, 1.0, interiorDistance);

        float shapeDist;
        if (glass.shapeType == 1) {
            float2 e = max(halfSize, float2(1e-4));
            float2 pe = localPos / e;
            float implicit = dot(pe, pe) - 1.0;
            shapeDist = max(-implicit, 0.0) * minHalf * 0.5;
            edgeFade = smoothstep(0.0, 1.0, shapeDist);
        } else {
            shapeDist = interiorDistance;
        }

        float bandScale = mix(0.25, 1.0, lb);
        float sRimW, sGleamW;
        if (glass.cursorActive > 0.5) {
            sRimW = max(mix(0.35, rimWidth * bandScale, si), 1.5);
            sGleamW = max(mix(0.75, gleamWidth * bandScale, si), 3.0);
        } else {
            sRimW = max(mix(0.35, fRimWidth * bandScale, si), 1.5);
            sGleamW = max(mix(0.75, fGleamWidth * bandScale, si), 3.0);
        }
        float sRimFalloff = exp(-shapeDist * shapeDist / (sRimW * sRimW));
        float sGleamFalloff = exp(-shapeDist * shapeDist / (sGleamW * sGleamW));

        float ambientBase = mix(0.15, 0.30, si);
        float liBright = mix(0.0, 1.0, li);

        float3 lightContrib = float3(0.0);

        if (glass.cursorActive > 0.5) {
            float2 pixelWorld = in.worldPos;
            float distToCursor = length(pixelWorld - glass.cursorWorldPos);

            float baseFalloff = max(length(halfSize) * 0.45, 120.0);
            float falloffScale = baseFalloff * mix(0.3, 1.0, li);
            float proximity = exp(-distToCursor * distToCursor / (falloffScale * falloffScale));

            // Element-level activation gate: how close is the cursor to this
            // element's center? Uses element diagonal radius so directional
            // lighting (directional, gleam, ghosts) fades to zero when the
            // cursor is more than ~1.5 element-widths away.
            float cursorToCenter = length(glass.cursorWorldPos - nodeCenter);
            float activationRadius = length(halfSize);
            float activation = exp(-cursorToCenter * cursorToCenter / (activationRadius * activationRadius));

            // Baseline rim light always visible (floor), boosted when cursor is near.
            float baselineFloor = ambientBase * sRimFalloff * edgeFade;
            baselineFloor *= variantLightScale * 0.6;
            float baselineActive = baselineFloor * activation;
            float baseline = max(baselineFloor * 0.7, baselineActive);

            float directional = proximity * sRimFalloff * edgeFade * activation;
            directional *= variantLightScale * 0.85 * liBright;

            float gleamScale = falloffScale * 1.8;
            float gleamProximity = exp(-distToCursor * distToCursor / (gleamScale * gleamScale));
            float gleam = gleamProximity * sGleamFalloff * edgeFade * activation;
            gleam *= variantLightScale * (0.25 * si) * liBright;

            lightContrib += (baseline + directional + gleam) * float3(1.0, 1.0, 1.05);

            // ── Edge-pinned lens ghosts ──────────────────────────────────────
            // Ghosts pinned to the glass boundary: march along the cursor→center
            // axis (both directions) to find where it intersects the shape edge,
            // then place anamorphic streak flares at those edge points. As the
            // cursor moves, the ghosts slide along the rim like light catching
            // the beveled edge of a crystal.
            // Cursor position in local (unrotated) space for ghost placement
            float2 cursorWorld = glass.cursorWorldPos - nodeCenter;
            float2 cursorLocal = float2(
                cursorWorld.x * cosR - cursorWorld.y * sinR,
                cursorWorld.x * sinR + cursorWorld.y * cosR
            );
            float cursorCenterDist = length(cursorLocal);

            // Axis from center toward cursor (and away from cursor)
            float2 axisDir = cursorCenterDist > 0.001
                ? cursorLocal / cursorCenterDist
                : float2(0.0, -1.0);

            // Edge squish setup (for per-pixel deformation near edges)
            float edgeSquish = mix(1.0, 3.5, edgeProximity);

            // Ghost proximity = activation (same Gaussian, same center distance)
            float orbProximity = activation;

            // Center diffusion: when cursor is near center, ghosts spread
            // along the edge contour (longer tangential reach) and the edge
            // band widens slightly — but light stays pinned to the boundary.
            // diffuse: 1.0 at center → 0.0 when cursor is far from center
            float diffuseThreshold = minHalf * 0.35;
            float diffuse = 1.0 - smoothstep(0.0, diffuseThreshold, cursorCenterDist);
            // Tangential spread: streaks wrap further around contour at center
            float diffuseTangentScale = mix(1.0, 4.0, diffuse);
            // Edge band: how far inward from the edge light is allowed to exist
            // lightBanding controls thickness: lb=0 sharp/thin, lb=1 soft/wide
            // Center diffusion widens the band further so ghosts connect
            float bandBase = mix(0.04, 0.20, lb);   // banding: thin → wide
            float bandDiffused = mix(0.10, 0.35, lb); // center: wider still
            float edgeBandWidth = mix(bandBase, bandDiffused, diffuse) * minHalf;
            float edgeBandSq = edgeBandWidth * edgeBandWidth;
            // Pixel's distance from edge — interiorDistance = max(-d, 0)
            float edgeBandMask = exp(-interiorDistance * interiorDistance / max(edgeBandSq, 1.0));
            // Squeeze relaxes slightly at center so streaks thicken a bit
            float diffuseSqueezeScale = mix(1.0, 0.4, diffuse);

            // Find edge intersection points by binary-search marching along
            // the axis in both directions (toward cursor and away from cursor).
            // Start at center (localPos=0), step outward until SDF goes positive.
            float2 edgeHitNear = float2(0.0);  // toward cursor
            float2 edgeHitFar = float2(0.0);   // away from cursor
            {
                // March toward cursor (+axisDir)
                float lo = 0.0;
                float hi = length(halfSize) * 1.2;
                for (int step = 0; step < 8; step++) {
                    float mid = (lo + hi) * 0.5;
                    float2 p = axisDir * mid;
                    float sd = sdGlassShape(p, halfSize, radii, glass);
                    if (sd > 0.0) { hi = mid; } else { lo = mid; }
                }
                edgeHitNear = axisDir * ((lo + hi) * 0.5);

                // March away from cursor (-axisDir)
                lo = 0.0;
                hi = length(halfSize) * 1.2;
                for (int step = 0; step < 8; step++) {
                    float mid = (lo + hi) * 0.5;
                    float2 p = -axisDir * mid;
                    float sd = sdGlassShape(p, halfSize, radii, glass);
                    if (sd > 0.0) { hi = mid; } else { lo = mid; }
                }
                edgeHitFar = -axisDir * ((lo + hi) * 0.5);
            }

            // Compute local surface tangent at each ghost's edge position.
            // The streak follows the contour of the shape at the pinned point.
            float ghostDelta = max(length(dfdx(localPos)), 0.5) * 3.0;

            // Near ghost tangent (SDF gradient → normal → tangent)
            float2 gNearGrad = float2(
                sdGlassShape(edgeHitNear + float2( ghostDelta, 0.0), halfSize, radii, glass)
              - sdGlassShape(edgeHitNear + float2(-ghostDelta, 0.0), halfSize, radii, glass),
                sdGlassShape(edgeHitNear + float2(0.0,  ghostDelta), halfSize, radii, glass)
              - sdGlassShape(edgeHitNear + float2(0.0, -ghostDelta), halfSize, radii, glass)
            );
            float2 gNearNorm = safeNormalize(gNearGrad);
            float2 gNearTangent = float2(-gNearNorm.y, gNearNorm.x);

            // Far ghost tangent
            float2 gFarGrad = float2(
                sdGlassShape(edgeHitFar + float2( ghostDelta, 0.0), halfSize, radii, glass)
              - sdGlassShape(edgeHitFar + float2(-ghostDelta, 0.0), halfSize, radii, glass),
                sdGlassShape(edgeHitFar + float2(0.0,  ghostDelta), halfSize, radii, glass)
              - sdGlassShape(edgeHitFar + float2(0.0, -ghostDelta), halfSize, radii, glass)
            );
            float2 gFarNorm = safeNormalize(gFarGrad);
            float2 gFarTangent = float2(-gFarNorm.y, gFarNorm.x);

            // Both ghosts use the same center-based proximity so the far
            // edge gleam isn't killed by distance falloff. Near ghost is
            // slightly brighter (1.3 vs 0.9) for realism.
            float2 ghostCenters[2] = { edgeHitNear, edgeHitFar };
            float2 ghostTangents[2] = { gNearTangent, gFarTangent };
            float2 ghostNormals[2] = { gNearNorm, gFarNorm };
            float ghostSizes[2] = { 1.0, 1.0 };
            float ghostBrights[2] = { 1.3, 0.9 };
            float ghostSqueezes[2] = { 10.0, 10.0 };

            float3 ghostContrib = float3(0.0);

            for (int gi = 0; gi < 2; gi++) {
                float2 toPixel = localPos - ghostCenters[gi];

                // Local tangent/normal at this ghost's edge position
                float2 ghostTan = ghostTangents[gi];
                float2 ghostNrm = ghostNormals[gi];

                // Decompose into tangent (along contour) and normal (across contour)
                float alongFlare = dot(toPixel, ghostTan);
                float acrossFlare = dot(toPixel, ghostNrm);

                // Edge deformation — use the ghost's own normal/tangent so the
                // squish is relative to the ghost's edge position, not the pixel's.
                // Using the pixel's surfaceNormal killed the far ghost because
                // the pixel and ghost normals point in opposite directions.
                float normalComp = dot(toPixel, ghostNrm);
                float tangentComp = dot(toPixel, ghostTan);
                float edgeDistSq = (normalComp * edgeSquish) * (normalComp * edgeSquish)
                                 + tangentComp * tangentComp;

                // Core: razor-thin bright line, tangential reach grows at center
                // bandScale ties streak length to lightBanding (same as base rim)
                float ghostBandScale = mix(0.25, 1.0, lb);
                float coreR = max(minHalf * 0.40 * ghostSizes[gi] * diffuseTangentScale * ghostBandScale, 24.0);
                float coreRSq = coreR * coreR;
                float cSqueeze = ghostSqueezes[gi] * diffuseSqueezeScale;
                float coreFlareDist = alongFlare * alongFlare
                                    + (acrossFlare * cSqueeze) * (acrossFlare * cSqueeze);
                float coreGlow = exp(-max(edgeDistSq, coreFlareDist) / coreRSq);

                // Glow: wider soft halo, tangential reach grows at center
                float glowR = max(minHalf * 1.00 * ghostSizes[gi] * diffuseTangentScale * ghostBandScale, 48.0);
                float glowRSq = glowR * glowR;
                float gSqueeze = max(cSqueeze * 0.3, 1.5);
                float glowFlareDist = alongFlare * alongFlare
                                    + (acrossFlare * gSqueeze) * (acrossFlare * gSqueeze);
                float glowFalloff = exp(-max(edgeDistSq, glowFlareDist) / glowRSq);

                float bright = ghostBrights[gi];
                float baseMul = orbProximity * si * liBright * variantLightScale * bright;

                float coreIntensity = coreGlow * baseMul * 2.0;
                float glowIntensity = glowFalloff * baseMul * 0.6;

                // Chromatic split along contour tangent (glow layer only)
                float chromaSpread = glass.dispersion * glowR * 0.35;
                float2 toPixelRc = toPixel - ghostTan * chromaSpread;
                float2 toPixelBc = toPixel + ghostTan * chromaSpread;

                float alongRc = dot(toPixelRc, ghostTan);
                float acrossRc = dot(toPixelRc, ghostNrm);
                float glowDistRc = alongRc*alongRc + (acrossRc*gSqueeze)*(acrossRc*gSqueeze);
                float nCompRc = dot(toPixelRc, ghostNrm);
                float tCompRc = dot(toPixelRc, ghostTan);
                float edgeDistSqRc = (nCompRc*edgeSquish)*(nCompRc*edgeSquish) + tCompRc*tCompRc;
                float chromaR = exp(-max(edgeDistSqRc, glowDistRc) / glowRSq);

                float alongBc = dot(toPixelBc, ghostTan);
                float acrossBc = dot(toPixelBc, ghostNrm);
                float glowDistBc = alongBc*alongBc + (acrossBc*gSqueeze)*(acrossBc*gSqueeze);
                float nCompBc = dot(toPixelBc, ghostNrm);
                float tCompBc = dot(toPixelBc, ghostTan);
                float edgeDistSqBc = (nCompBc*edgeSquish)*(nCompBc*edgeSquish) + tCompBc*tCompBc;
                float chromaB = exp(-max(edgeDistSqBc, glowDistBc) / glowRSq);

                float glowMul = orbProximity * si * liBright * 0.6 * variantLightScale * bright;

                // White core + chromatic glow, masked to edge band
                float3 thisGhost = float3(coreIntensity)
                                 + float3(chromaR * glowMul, glowIntensity, chromaB * glowMul);
                ghostContrib += thisGhost * edgeBandMask;
            }

            lightContrib += ghostContrib;

        } else {
            float angleDelta = pixelAngle - glass.lightAngle;
            angleDelta = angleDelta - 6.2831853 * floor((angleDelta + 3.1415927) / 6.2831853);
            float angularCos = cos(angleDelta);

            float hotspotExp = mix(8.0, 2.0, li);
            float hotspot = pow(max(angularCos, 0.0), hotspotExp);

            float gleamExp = mix(5.0, 1.0, li);
            float gleamSpot = pow(max(angularCos, 0.0), gleamExp);

            float baseline = ambientBase * sRimFalloff * edgeFade;
            baseline *= variantLightScale * 0.6;

            float directional = hotspot * sRimFalloff * edgeFade;
            directional *= variantLightScale * 0.85 * fresnelBoost * liBright;

            float gleam = gleamSpot * sGleamFalloff * edgeFade;
            gleam *= variantLightScale * (0.25 * si) * fresnelBoost * liBright;

            lightContrib += (baseline + directional + gleam) * float3(1.0, 1.0, 1.05);

            // Interior caustic: light focused deeper inside the glass body
            float interiorMask = smoothstep(0.05, 0.4, edgeProximityNorm);
            float causticAngle = max(angularCos, 0.0);
            float causticStrength = interiorMask * causticAngle * si * 0.12 * liBright;
            lightContrib += causticStrength * float3(1.0, 1.0, 1.03);
        }

        // ── Brilliance source rim highlights ──────────────────────────────
        // When Brilliance lights are present, add directional rim glow facing
        // each light origin. Both pixel and source angles computed in world
        // space so rotation transforms are bypassed entirely.
        if (glass.brillianceCount > 0) {
            float2 bSources[4] = {
                glass.brillianceSource0, glass.brillianceSource1,
                glass.brillianceSource2, glass.brillianceSource3
            };
            float3 bTints[4] = {
                glass.brillianceTint0, glass.brillianceTint1,
                glass.brillianceTint2, glass.brillianceTint3
            };
            float2 pixelRel = in.worldPos - nodeCenter;
            float wPixelAngle = atan2(pixelRel.x, -pixelRel.y);
            for (uint bi = 0; bi < glass.brillianceCount && bi < 4; bi++) {
                float2 srcRel = bSources[bi] - nodeCenter;
                float srcLen = length(srcRel);
                if (srcLen < 0.001) continue;
                float srcAngle = atan2(srcRel.x, -srcRel.y);
                float srcAngleDelta = wPixelAngle - srcAngle;
                srcAngleDelta = srcAngleDelta - 6.2831853 * floor((srcAngleDelta + 3.1415927) / 6.2831853);
                float srcAngularCos = cos(srcAngleDelta);
                float srcHotspot = pow(max(srcAngularCos, 0.0), mix(8.0, 2.0, li));
                float srcGleamSpot = pow(max(srcAngularCos, 0.0), mix(5.0, 1.0, li));
                // Fade as source moves away from glass
                float srcProxFade = 1.0 - smoothstep(minHalf * 1.0, minHalf * 6.0, srcLen);
                if (srcProxFade < 0.001) continue;

                // Per-light rim tint: explicit tint > resonance > canvas sampling
                float3 rimTint;
                if (glass.tintOpacity > 0.001) {
                    rimTint = glass.tintColor.rgb;
                } else if (glass.resonanceEnabled != 0) {
                    rimTint = glass.smoothedResonanceTint;
                } else {
                    rimTint = bTints[bi];
                }
                float3 rimColor = mix(float3(1.0, 1.0, 1.05), rimTint, 0.35);

                float bDir = srcHotspot * sRimFalloff * edgeFade
                           * variantLightScale * 0.85 * fresnelBoost * liBright * srcProxFade;
                float bGleam = srcGleamSpot * sGleamFalloff * edgeFade
                             * variantLightScale * (0.25 * si) * fresnelBoost * liBright * srcProxFade;
                lightContrib += (bDir + bGleam) * rimColor;
            }
        }

        float lightAlpha = max(max(lightContrib.r, lightContrib.g), lightContrib.b);
        lightAlpha = clamp(lightAlpha, 0.0, 1.0);
        return float4(lightContrib, lightAlpha) * shapeMask;
    }

    // ── Composite Pass ──────────────────────────────────────────────────
    fragment float4 fragment_glass_composite(
        GlassVertexOut in [[stage_in]],
        texture2d<float> blurredBg [[texture(0)]],
        texture2d<float> sharpBg [[texture(1)]],
        texture2d<float> maskTex [[texture(2)]],
        texture2d<float> rimLightTex [[texture(3)]],
        texture2d<float> glowTex [[texture(4)]],
        constant HolodeckCompositeUniforms &glass [[buffer(0)]]
    ) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);

        float2 quadSize = glass.size + float2(glass.brillianceMargin * 2.0);
        float2 localPos = (in.uv - 0.5) * quadSize;
        float2 halfSize = glass.size * 0.5;
        float4 radii = min(glass.cornerRadii, float4(min(halfSize.x, halfSize.y)));
        float d = sdGlassShape(localPos, halfSize, radii, glass);

        float2 dpdx_lp = dfdx(localPos);
        float2 dpdy_lp = dfdy(localPos);
        float pixelSize = length(float2(length(dpdx_lp), length(dpdy_lp))) * 0.7071;
        float aa = max(pixelSize, 0.75);
        float shapeMask = 1.0 - smoothstep(-aa, aa, d);

        float interiorDistance = max(-d, 0.0);

        if (glass.shapeType == 1) {
            float2 eSmooth = max(halfSize, float2(1e-4));
            float2 peSmooth = localPos / eSmooth;
            float minHalfE = min(halfSize.x, halfSize.y);
            interiorDistance = max(-(dot(peSmooth, peSmooth) - 1.0), 0.0) * minHalfE * 0.5;
        }

        float falloffFrac = mix(0.01, 0.60, glass.edgeWidth);
        float falloffPx = max(6.0, min(halfSize.x, halfSize.y) * falloffFrac);
        float edgeProximity = 1.0 - smoothstep(0.0, falloffPx, interiorDistance);
        float lensProfile = edgeProximity;

        bool hasSdfTexture = glass.maskConfig.z > 0.5;
        bool useSDFTexture = hasSdfTexture && (glass.shapeType != 3);
        float2 sdfMaskPadding = float2(0.0);
        float2 sdfMaskRange = float2(1.0);
        float sdfTexelToPoint = 1.0;
        float2 sdfUV = float2(0.0);
        if (useSDFTexture) {
            sdfMaskPadding = clamp(glass.maskConfig.xy, 0.0, 0.49);
            sdfMaskRange = max(1.0 - 2.0 * sdfMaskPadding, float2(1e-4));
            sdfUV = clamp(sdfMaskPadding + in.uv * sdfMaskRange, 0.0, 1.0);
            float2 sdfTexSize = float2(float(maskTex.get_width()), float(maskTex.get_height()));

            float2 texelsPerPt = (sdfTexSize * sdfMaskRange) / max(glass.size, float2(1.0));
            sdfTexelToPoint = 1.0 / max(0.5 * (texelsPerPt.x + texelsPerPt.y), 1e-4);

            float4 sdfSample = maskTex.sample(s, sdfUV);
            float sdfValue = sdfSample.r;

            // UV-space AA — same approach as rim light pass for consistency.
            float2 sdfUVdx = dfdx(sdfUV);
            float2 sdfUVdy = dfdy(sdfUV);
            float texelFootprint = length(float2(length(sdfUVdx), length(sdfUVdy)))
                                 * max(sdfTexSize.x, sdfTexSize.y) * 0.7071;
            float sdfAA = max(texelFootprint, 0.75);
            float refinedMask = 1.0 - smoothstep(-sdfAA, sdfAA, sdfValue);
            shapeMask = refinedMask;

            float sdfPoints = sdfValue * sdfTexelToPoint;
            float sdfInterior = max(-sdfPoints, 0.0);
            float heightVal = sdfSample.a;
            float heightInterior = heightVal * sdfTexelToPoint;

            float edgeBlend = smoothstep(3.0, 6.0, sdfInterior);
            interiorDistance = mix(sdfInterior, heightInterior, edgeBlend);
            edgeProximity = 1.0 - smoothstep(0.0, falloffPx, interiorDistance);
            float maxDepthEst = min(halfSize.x, halfSize.y) * 0.5;
            float depthFrac = clamp(interiorDistance / max(maxDepthEst, 1.0), 0.0, 1.0);
            lensProfile = 1.0 - depthFrac;
        }

        float3 flare = float3(0.0);

        if (shapeMask < 0.001 && glass.brillianceCount == 0) {
            return float4(0.0);
        }

        float minHalf = min(halfSize.x, halfSize.y);
        float maxInteriorDist = minHalf * 0.5;

        // Cap refraction displacement so large shapes don't get proportionally
        // enormous distortion. Small shapes (minHalf < 60) are unaffected.
        float refractionMaxDist = min(maxInteriorDist, 30.0);

        float edgeExp = mix(6.0, 2.0, glass.edgeWidth);

        // Surface normal at this pixel (outward-facing from nearest edge).
        // Computed alongside refraction and reused for Fresnel rim lighting.
        float2 surfaceNormal = float2(0.0, -1.0);

        float2 refractVec;
        if (glass.shapeType == 1) {
            float2 normEllipse = localPos / max(halfSize, float2(1e-4));
            float radial = clamp(length(normEllipse), 0.0, 1.0);
            float edgeWeight = refractionFalloff(1.0 - radial, edgeExp);

            float2 e = max(halfSize, float2(1e-4));
            float2 outward = float2(localPos.x / (e.x * e.x), localPos.y / (e.y * e.y));
            float2 inwardDir = -safeNormalize(outward);
            surfaceNormal = safeNormalize(outward);

            refractVec = inwardDir * edgeWeight * refractionMaxDist * glass.refractionStrength;
        } else if (glass.shapeType == 3) {
            float starMinHalf = min(halfSize.x, halfSize.y);
            float2 starScale = halfSize / max(starMinHalf, 1e-4);
            float2 starPos = localPos / starScale;
            float outerR = starMinHalf * glass.outerRadius;
            float innerR = outerR * glass.innerRadius;
            float midR = (outerR + innerR) * 0.5;
            float ampR = (outerR - innerR) * 0.5;

            float n = float(max(glass.sides, 3u));

            float delta = max(length(dfdx(starPos)), 0.5) * 2.0;

            float2 sp1 = starPos + float2(delta, 0.0);
            float a1 = atan2(sp1.x, sp1.y);
            float br1 = midR + ampR * cos(n * a1);
            float wr1 = length(sp1) / max(br1, 1e-4);

            float2 sp2 = starPos + float2(-delta, 0.0);
            float a2 = atan2(sp2.x, sp2.y);
            float br2 = midR + ampR * cos(n * a2);
            float wr2 = length(sp2) / max(br2, 1e-4);

            float2 sp3 = starPos + float2(0.0, -delta);
            float a3 = atan2(sp3.x, sp3.y);
            float br3 = midR + ampR * cos(n * a3);
            float wr3 = length(sp3) / max(br3, 1e-4);

            float2 sp4 = starPos + float2(0.0, delta);
            float a4 = atan2(sp4.x, sp4.y);
            float br4 = midR + ampR * cos(n * a4);
            float wr4 = length(sp4) / max(br4, 1e-4);

            float2 warpGrad = float2(wr1 - wr2, wr4 - wr3);
            float warpGradMag = length(warpGrad);
            float2 inwardDir = (warpGradMag > 1e-5) ? (-warpGrad / warpGradMag) : float2(0.0);
            surfaceNormal = (warpGradMag > 1e-5) ? (warpGrad / warpGradMag) : float2(0.0, -1.0);

            // Fade refraction where the gradient is weak (medial axis / ridge line).
            // The gradient drops toward zero between star arms where two edges
            // are equidistant — that's exactly where the seam appears.
            float starGradWeight = smoothstep(0.01, 0.12, warpGradMag);

            float starDepth = max(-d, 0.0);
            float nd = clamp(starDepth / max(maxInteriorDist, 1.0), 0.0, 1.0);
            float edgeWeight = refractionFalloff(nd, edgeExp);

            refractVec = inwardDir * edgeWeight * starGradWeight * refractionMaxDist * glass.refractionStrength;
        } else if (glass.shapeType == 2) {
            // ── Polygon: vertex-based nearest-edge normal ──
            // The generic SDF finite-difference gradient creates an "X" on
            // 4-sided polygons because atan2 has discontinuities at sector
            // boundaries. Instead, iterate the polygon edges explicitly to
            // find the nearest one and use its geometric outward normal.
            float polyMinHalf = min(halfSize.x, halfSize.y);
            float2 polyScale = halfSize / max(polyMinHalf, 1e-4);
            float2 polyPos = localPos / polyScale;

            uint pn = max(glass.sides, 3u);
            float fn = float(pn);
            float pan = 3.14159265 / fn;

            // Find two nearest edges for medial axis detection
            float bestDist = 1e10;
            float secondBestDist = 1e10;
            float2 bestNormal = float2(0.0, -1.0);
            for (uint i = 0; i < pn; i++) {
                // Vertices placed with atan2(x,y) convention so even-sided
                // polygons have a top vertex (matching sdPolygon orientation).
                float a0 = float(i) * 2.0 * pan;
                float a1 = float(i + 1) * 2.0 * pan;
                float2 v0 = polyMinHalf * float2(sin(a0), cos(a0));
                float2 v1 = polyMinHalf * float2(sin(a1), cos(a1));

                // Edge midpoint normal (outward-facing)
                float2 edgeMid = 0.5 * (v0 + v1);
                float2 eNorm = safeNormalize(edgeMid);

                // Distance from point to the edge line (not segment — the
                // polygon is convex so projecting onto the infinite line is fine
                // for choosing the nearest face).
                float d = dot(polyPos - v0, eNorm);
                // d is signed: positive = outside edge, negative = inside
                float absDist = abs(d);
                if (absDist < bestDist) {
                    secondBestDist = bestDist;
                    bestDist = absDist;
                    bestNormal = eNorm;
                } else if (absDist < secondBestDist) {
                    secondBestDist = absDist;
                }
            }

            // Medial axis fade: when two edges are nearly equidistant,
            // we're on the seam. Fade refraction based on how "decisive"
            // the nearest-edge choice is. edgeRatio → 1 on the medial axis.
            float edgeRatio = bestDist / max(secondBestDist, 1e-4);
            float polyGradWeight = smoothstep(0.85, 0.98, edgeRatio);
            polyGradWeight = 1.0 - polyGradWeight;  // 1 = clear winner, 0 = on the seam

            // Un-scale normal back to local space
            bestNormal = safeNormalize(bestNormal * polyScale);
            surfaceNormal = bestNormal;

            float2 inwardDir = -bestNormal;
            float2 centroidDir = -safeNormalize(localPos);

            float nd = clamp(interiorDistance / max(maxInteriorDist, 1.0), 0.0, 1.0);
            float blendToCenter = smoothstep(0.15, 0.4, nd);
            inwardDir = normalize(mix(inwardDir, centroidDir, blendToCenter));

            float edgeWeight = refractionFalloff(nd, edgeExp);

            refractVec = inwardDir * edgeWeight * polyGradWeight * refractionMaxDist * glass.refractionStrength;
        } else if (useSDFTexture) {
            float2 sdfTexSize = float2(float(maskTex.get_width()), float(maskTex.get_height()));
            float2 texelSize = 1.0 / max(sdfTexSize, float2(1.0));

            float hL = maskTex.sample(s, sdfUV + float2(-texelSize.x, 0.0)).a;
            float hR = maskTex.sample(s, sdfUV + float2( texelSize.x, 0.0)).a;
            float hT = maskTex.sample(s, sdfUV + float2(0.0, -texelSize.y)).a;
            float hB = maskTex.sample(s, sdfUV + float2(0.0,  texelSize.y)).a;
            float2 heightGrad = float2(hR - hL, hB - hT) * 0.5;
            float gradMag = length(heightGrad);
            float2 inwardDir = (gradMag > 1e-5) ? (heightGrad / gradMag) : float2(0.0);
            surfaceNormal = (gradMag > 1e-5) ? (-heightGrad / gradMag) : float2(0.0, -1.0);

            float gradWeight = smoothstep(0.005, 0.12, gradMag);
            float sdfDepth = 1.0 - clamp(edgeProximity, 0.0, 1.0);
            float edgeWeight = refractionFalloff(sdfDepth, max(1.0, edgeExp - 0.5));

            refractVec = inwardDir * edgeWeight * gradWeight * refractionMaxDist * glass.refractionStrength;
        } else {
            float delta = max(length(dfdx(localPos)), 0.5) * 2.0;
            float dR = sdGlassShape(localPos + float2( delta, 0.0), halfSize, radii, glass);
            float dL = sdGlassShape(localPos + float2(-delta, 0.0), halfSize, radii, glass);
            float dU = sdGlassShape(localPos + float2(0.0, -delta), halfSize, radii, glass);
            float dD = sdGlassShape(localPos + float2(0.0,  delta), halfSize, radii, glass);
            float2 sdfGrad = float2(dR - dL, dD - dU);
            float sdfGradMag = length(sdfGrad);
            float2 edgeDir = (sdfGradMag > 1e-5) ? (-sdfGrad / sdfGradMag) : float2(0.0);
            surfaceNormal = (sdfGradMag > 1e-5) ? (sdfGrad / sdfGradMag) : float2(0.0, -1.0);

            // Fade refraction where the SDF gradient is weak (medial axis).
            // Widen the fade at high edgeWidth where refraction extends deeper
            // into the interior and medial axis artifacts become visible.
            float gradFadeEnd = mix(0.25, 0.45, glass.edgeWidth);
            float rectGradWeight = smoothstep(0.01, gradFadeEnd, sdfGradMag / max(delta, 1e-4));

            float2 centroidDir = -safeNormalize(localPos);

            float nd = clamp(interiorDistance / max(maxInteriorDist, 1.0), 0.0, 1.0);
            // Blend to centroid direction so the SDF gradient's 90° rotation
            // at corners is smoothed out before it can create visible knots.
            // At high edgeWidth, transition almost immediately to centroid
            // direction — the SDF gradient is only reliable very close to edges.
            float blendEnd = mix(0.25, 0.10, glass.edgeWidth);
            float blendToCenter = smoothstep(0.05, blendEnd, nd);
            float2 inwardDir = normalize(mix(edgeDir, centroidDir, blendToCenter));

            float edgeWeight = refractionFalloff(nd, edgeExp);

            refractVec = inwardDir * edgeWeight * rectGradWeight * refractionMaxDist * glass.refractionStrength;
        }

        // Blend refraction magnitude with lensProfile so it shares the same
        // spatial envelope as the rim light / edge band. At high edgeWidth,
        // both effects fade together instead of creating a hard boundary.
        // Clamp to non-negative — the Gaussian falloff already handles
        // thinning at negative edgeWidth; without the clamp, negative values
        // invert the mix and amplify refraction instead.
        float lensBlend = lensProfile * lensProfile;  // squared for smoother rolloff
        refractVec *= mix(1.0, lensBlend, max(glass.edgeWidth, 0.0));

        float variantRefractionScale = (glass.glassVariant == 1) ? 0.65 : 1.0;
        float2 refractUV = refractVec * variantRefractionScale;

        float frostAmount = glass.frost;
        frostAmount = clamp(frostAmount, 0.0, 1.0);

        // Map screenUV [0,1] to the glass region within the full backdrop texture.
        // cropUVOffset/cropUVScale are floating-point — no integer snap, no jitter.
        // When the texture IS the crop (legacy/blur-padded path), offset=blurPadding
        // and scale=paddedRange, which recovers the original behavior exactly.
        float2 cropScale = glass.cropUVScale;
        float2 cropOffset = glass.cropUVOffset;
        float2 texCenter = cropOffset + cropScale * 0.5;

        float2 baseUV = cropOffset + in.screenUV * cropScale;

        float2 splayOffset = baseUV - texCenter;
        float r = length(splayOffset / (cropScale * 0.5));
        float splayStrength = abs(glass.splayStrength);
        float splayFactor = 1.0 + splayStrength * r * r;
        baseUV = texCenter + splayOffset * splayFactor;

        float cosRot = cos(glass.rotation);
        float sinRot = sin(glass.rotation);
        refractUV = float2(
            refractUV.x * cosRot - refractUV.y * sinRot,
            refractUV.x * sinRot + refractUV.y * cosRot
        );

        refractUV *= cropScale;

        float2 texel = 1.0 / float2(float(blurredBg.get_width()), float(blurredBg.get_height()));
        float2 uvInset = min(texel, float2(0.49));
        float2 uvMin = cropOffset + uvInset;
        float2 uvMax = cropOffset + cropScale - uvInset;

        // ── Edge parallax ──
        // Slide the background sample inward (away from the nearest canvas edge)
        // as the shape moves off-screen — like a glass window panning over a
        // fixed scene. Fragments near the edge sample from deeper inside the
        // texture instead of piling up at the boundary.
        float2 edgeParallax;
        {
            // Scale by zoom and shape size — bigger shapes need more parallax.
            float maxShift = 0.04 * max(glass.canvasZoom, 1.0) * max(glass.viewportScale, 0.1);

            // Use shape's viewport position — uniform shift, no per-fragment stretching.
            // Sine curve: steepest change near center, slopes off toward edges
            // so the parallax shift decelerates as you approach the canvas frame.
            float2 edgeness = (glass.viewportCenter - 0.5) * 2.0;  // [-1, 1] per axis
            float2 strength = sin(edgeness * M_PI_F / 2.0);  // sine: fast center, gentle edges

            edgeParallax = -strength * maxShift;
        }

        float2 radialDir = safeNormalize(baseUV - texCenter);
        float radialDist = length((baseUV - texCenter) / max(cropScale * 0.5, float2(1e-4)));
        float radialStrength = mix(0.3, 1.0, clamp(radialDist, 0.0, 1.0));
        // Chromatic fringe direction: near edges follows the surface normal
        // (rainbow hugs the contour), interior falls back to radial direction.
        float2 radialAberration = radialDir * radialStrength * glass.dispersion * 0.15;
        float2 normalAberration = surfaceNormal * glass.dispersion * 0.15;
        float normalInfluence = smoothstep(0.0, falloffPx * 0.5, interiorDistance);
        float2 aberrationDir = mix(normalAberration, radialAberration, normalInfluence);

        float3 bgSampled;
        if (glass.dispersion < 0.001) {
            // Fast path: no chromatic aberration, single texture sample.
            float2 zoomedUV = texCenter + (baseUV + refractUV - texCenter) / glass.depthScale;
            zoomedUV += edgeParallax;
            zoomedUV = clamp(zoomedUV, uvMin, uvMax);
            bgSampled = blurredBg.sample(s, zoomedUV).rgb;
        } else {
            float3 spectralSum = float3(0.0);
            float3 weightSum = float3(0.0);
            for (int band = 0; band < 7; band++) {
                float t = float(band) / 6.0;
                float offset = t * 2.0 - 1.0;
                float2 bandUV = baseUV + refractUV + aberrationDir * offset;
                float2 zoomedUV = texCenter + (bandUV - texCenter) / glass.depthScale;
                zoomedUV += edgeParallax;
                zoomedUV = clamp(zoomedUV, uvMin, uvMax);
                float3 samp = blurredBg.sample(s, zoomedUV).rgb;
                float3 w = spectralW[band];
                spectralSum += samp * w;
                weightSum += w;
            }
            bgSampled = spectralSum / max(weightSum, float3(1e-4));
        }

        float fresnelWidth = clamp(minHalf * 0.08, 4.0, 20.0);
        float fresnelZone = 1.0 - smoothstep(0.0, fresnelWidth, interiorDistance);
        float tilt = clamp(fresnelZone, 0.0, 0.999);
        float cosTheta = sqrt(1.0 - tilt * tilt);

        float F0 = 0.04;
        float oneMinusCos = 1.0 - cosTheta;
        float oneMinusCos2 = oneMinusCos * oneMinusCos;
        float fresnel = F0 + (1.0 - F0) * (oneMinusCos2 * oneMinusCos2 * oneMinusCos);

        fresnel *= clamp(glass.refractionStrength * 8.0, 0.0, 0.5);

        float3 reflColor = float3(0.0);
        if (fresnel > 0.001) {
            float2 rawRefl = baseUV - refractUV * 1.5;
            float2 uvRefl = texCenter + (rawRefl - texCenter) / glass.depthScale;
            uvRefl = clamp(uvRefl, uvMin, uvMax);

            float4 reflSample = blurredBg.sample(s, uvRefl);
            reflColor = reflSample.rgb;
        }

        float3 bgColor = bgSampled;

        bgColor = mix(bgColor, reflColor, fresnel);

        if (frostAmount > 0.01) {
            float shimmerEdge = exp(-interiorDistance * interiorDistance / (max(minHalf * 0.1, 4.0) * max(minHalf * 0.1, 4.0)));
            float shimmerStrength = shimmerEdge * shimmerEdge * frostAmount * glass.lightIntensity * 0.12;
            bgColor += float3(shimmerStrength);
        }

        // ── Tint / Resonance ─────────────────────────────────────────────
        float3 resonanceTint = float3(1.0);
        if (glass.resonanceEnabled != 0) {
            // Compute fresh tint from background probes each frame.
            float2 probeSpan = cropScale * 0.3;
            float3 samples[5];
            samples[0] = blurredBg.sample(s, texCenter).rgb;
            samples[1] = blurredBg.sample(s, texCenter + float2( probeSpan.x, 0)).rgb;
            samples[2] = blurredBg.sample(s, texCenter + float2(-probeSpan.x, 0)).rgb;
            samples[3] = blurredBg.sample(s, texCenter + float2(0,  probeSpan.y)).rgb;
            samples[4] = blurredBg.sample(s, texCenter + float2(0, -probeSpan.y)).rgb;
            float avgLum = 0.0;
            float3 darkest = float3(1.0), lightest = float3(0.0);
            for (int i = 0; i < 5; i++) {
                float lum = dot(samples[i], float3(0.2126, 0.7152, 0.0722));
                avgLum += lum;
                darkest = min(darkest, samples[i]);
                lightest = max(lightest, samples[i]);
            }
            avgLum /= 5.0;
            float3 freshTint = mix(lightest, darkest, avgLum);

            // Temporal smoothing: blend fresh probe with CPU anchor.
            // blendFactor ~0.08 means 92% stable anchor + 8% fresh → no flicker.
            resonanceTint = (glass.resonanceBlendFactor > 0.001)
                ? mix(glass.smoothedResonanceTint, freshTint, glass.resonanceBlendFactor)
                : freshTint;

            if (glass.tintOpacity > 0.001) {
                resonanceTint = mix(resonanceTint, glass.tintColor.rgb, 0.5);
            }
            float resOpacity = (glass.tintOpacity > 0.001) ? glass.tintOpacity : 0.15;
            bgColor = mix(bgColor, resonanceTint, resOpacity);
        } else if (glass.tintOpacity > 0.001) {
            float3 tint = glass.tintColor.rgb;
            bgColor = mix(bgColor, tint, glass.tintOpacity);
        }

        // ── Inner light/shadow (luminance-driven) ─────────────────────
        if (glass.luminanceEnabled) {
            float bgLum = dot(bgSampled, float3(0.2126, 0.7152, 0.0722));

            float lightAmount = smoothstep(0.4, 0.6, bgLum);
            float shadowAmount = smoothstep(0.5, 0.3, bgLum);

            bgColor += float3(1.0, 0.98, 0.95) * lightAmount * 0.15;
            bgColor *= 1.0 - shadowAmount * 0.12;
        }

        // ── Brilliance (user-specified light sources → lens flare) ────
        // Light source positions come directly from uniforms (set by user).
        // Each source contributes an independent flare: star burst diffraction
        // spikes at the entry point + chromatic ghost orbs on the opposite side.
        // Modulated by depth (ghost size), frost (diffusion), tint/resonance (color).
        if (glass.brillianceCount > 0) {
            float mh = max(minHalf, 1.0);
            float2 glassCenter = in.worldPos - localPos;
            float ghostDelta = max(mh * 0.01, 1.0);

            // Property modulation
            float depthGhostScale = sqrt(max(glass.depthScale, 0.5));
            float frostShrink = 1.0 - frostAmount * 0.5;
            float frostDim = 1.0 - frostAmount * 0.6;
            float ghostScale = depthGhostScale * frostShrink;

            // Per-light tint arrays for adaptive canvas sampling
            float3 bTints[4] = {
                glass.brillianceTint0, glass.brillianceTint1,
                glass.brillianceTint2, glass.brillianceTint3
            };

            // Gather up to 4 source positions from uniforms
            float2 sources[4] = {
                glass.brillianceSource0, glass.brillianceSource1,
                glass.brillianceSource2, glass.brillianceSource3
            };

            for (uint si = 0; si < glass.brillianceCount && si < 4; si++) {
                float2 lightLocal = sources[si] - glassCenter;
                float lpLen = length(lightLocal);
                float2 flareAxis = lpLen > 0.001 ? lightLocal / lpLen : float2(0.0, -1.0);

                // Collapse: ghosts shrink inward as light approaches center
                float collapseFactor = smoothstep(0.0, mh * 0.3, lpLen);

                // Intensity fades as source moves far from the glass
                float proxFade = 1.0 - smoothstep(mh * 1.0, mh * 6.0, lpLen);
                if (proxFade < 0.001) continue;

                // Per-light tint: explicit tint overrides resonance overrides canvas sampling
                float3 perLightTint;
                float perLightTintStr;
                if (glass.tintOpacity > 0.001) {
                    perLightTint = glass.tintColor.rgb;
                    perLightTintStr = glass.tintOpacity * 0.5;
                } else if (glass.resonanceEnabled != 0) {
                    perLightTint = resonanceTint;
                    perLightTintStr = 0.5;
                } else {
                    perLightTint = bTints[si];
                    perLightTintStr = 0.5;
                }
                float3 flareTintMul = mix(float3(1.0), perLightTint, perLightTintStr);

                // Spatial early-out cutoff: skip ghost SDF samples when pixel
                // is too far for the Gaussian to contribute (< 0.001).
                float maxGhostFalloff = useSDFTexture ? (mh * 0.36) : (mh * 3.15);
                bool chromaticGhosts = glass.dispersion >= 0.001;

                // Ghost 1: close to center on opposite side
                float2 g1 = flareAxis * (-0.35 * mh * ghostScale * collapseFactor);
                float r1 = mh * 0.14 * ghostScale * collapseFactor;
                float cOff1 = glass.dispersion * mh * 0.04 * collapseFactor;
                if (length(localPos - g1) < r1 + cOff1 + maxGhostFalloff) {
                    float3 g1Color;
                    if (chromaticGhosts) {
                        float g1G = shapeGhost(localPos, g1, r1, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g1R = shapeGhost(localPos, g1 + flareAxis * cOff1, r1, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g1B = shapeGhost(localPos, g1 - flareAxis * cOff1, r1, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g1Color = float3(g1R, g1G, g1B);
                    } else {
                        float g1v = shapeGhost(localPos, g1, r1, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g1Color = float3(g1v);
                    }
                    flare += g1Color * 0.35 * proxFade * frostDim * collapseFactor * flareTintMul;
                }

                // Ghost 2: mid-distance opposite
                float2 g2 = flareAxis * (-0.7 * mh * ghostScale * collapseFactor);
                float r2 = mh * 0.10 * ghostScale * collapseFactor;
                float cOff2 = glass.dispersion * mh * 0.06 * collapseFactor;
                if (length(localPos - g2) < r2 + cOff2 + maxGhostFalloff) {
                    float3 g2Color;
                    if (chromaticGhosts) {
                        float g2G = shapeGhost(localPos, g2, r2, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g2R = shapeGhost(localPos, g2 + flareAxis * cOff2, r2, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g2B = shapeGhost(localPos, g2 - flareAxis * cOff2, r2, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g2Color = float3(g2R, g2G, g2B);
                    } else {
                        float g2v = shapeGhost(localPos, g2, r2, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g2Color = float3(g2v);
                    }
                    flare += g2Color * 0.25 * proxFade * frostDim * collapseFactor * flareTintMul;
                }

                // Ghost 3: ring near center
                float2 g3 = flareAxis * (-0.2 * mh * ghostScale * collapseFactor);
                float r3 = mh * 0.09 * ghostScale * collapseFactor;
                if (length(localPos - g3) < r3 + maxGhostFalloff) {
                    float ringW = r3 * 0.2;
                    float ring = shapeRing(localPos, g3, r3, ringW, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                    flare += float3(0.5, 0.7, 1.0) * flareTintMul * ring * 0.2 * proxFade * frostDim * collapseFactor;
                }

                // Ghost 4: far opposite wash
                float2 g4 = flareAxis * (-1.0 * mh * ghostScale * collapseFactor);
                float r4 = mh * 0.18 * ghostScale * collapseFactor;
                if (length(localPos - g4) < r4 + maxGhostFalloff) {
                    float g4v = shapeGhost(localPos, g4, r4, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                    flare += float3(0.7, 0.85, 1.0) * flareTintMul * g4v * 0.18 * proxFade * frostDim * collapseFactor;
                }

                // Ghost 5: tiny chromatic point near opposite edge
                float2 g5 = flareAxis * (-1.2 * mh * ghostScale * collapseFactor);
                float r5 = mh * 0.05 * ghostScale * collapseFactor;
                float cOff5 = glass.dispersion * mh * 0.08 * collapseFactor;
                if (length(localPos - g5) < r5 + cOff5 + maxGhostFalloff) {
                    float3 g5Color;
                    if (chromaticGhosts) {
                        float g5R = shapeGhost(localPos, g5 + flareAxis * cOff5, r5, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g5G = shapeGhost(localPos, g5, r5, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        float g5B = shapeGhost(localPos, g5 - flareAxis * cOff5, r5, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g5Color = float3(g5R, g5G, g5B);
                    } else {
                        float g5v = shapeGhost(localPos, g5, r5, halfSize, radii, ghostDelta, mh, glass, maskTex, s, useSDFTexture, sdfMaskPadding, sdfMaskRange, sdfTexelToPoint, quadSize);
                        g5Color = float3(g5v);
                    }
                    flare += g5Color * 0.3 * proxFade * frostDim * collapseFactor * flareTintMul;
                }
            }

        }

        // ── Rim lighting (sampled from pre-rendered texture) ────────────
        // The rim light was rendered to rimLightTex in a separate pass.
        // The rim texture covers the axis-aligned bounding box of the rotated
        // quad, so we must rotate localPos to world orientation and remap into
        // the AABB-sized texture UV space.
        {
            constexpr sampler rimSampler(mag_filter::linear, min_filter::linear, address::clamp_to_edge);

            // AABB of the rotated node (same math as Holodeck.swift)
            // Reuse cosRot/sinRot already computed above
            float absC = abs(cosRot);
            float absS = abs(sinRot);
            float2 aabb = float2(
                glass.size.x * absC + glass.size.y * absS,
                glass.size.x * absS + glass.size.y * absC
            );

            // Rotate localPos into world orientation, then map into AABB UV
            float cosRc = cosRot;
            float sinRc = sinRot;
            float2 rotLP = float2(
                localPos.x * cosRc - localPos.y * sinRc,
                localPos.x * sinRc + localPos.y * cosRc
            );
            float2 rimBaseUV = rotLP / max(aabb, float2(1.0)) + 0.5;

            // Apply refraction offset (convert local-space refractVec to rim UV)
            float2 rotRefract = float2(
                refractVec.x * cosRc - refractVec.y * sinRc,
                refractVec.x * sinRc + refractVec.y * cosRc
            );
            rimBaseUV -= rotRefract * variantRefractionScale / max(aabb, float2(1.0));

            // Chromatic dispersion: offset R and B along the surface normal
            float2 rotNormal = float2(
                surfaceNormal.x * cosRc - surfaceNormal.y * sinRc,
                surfaceNormal.x * sinRc + surfaceNormal.y * cosRc
            );
            float2 rimDispDir = rotNormal * glass.dispersion * 0.08;
            float rimR = rimLightTex.sample(rimSampler, rimBaseUV - rimDispDir).r;
            float rimG = rimLightTex.sample(rimSampler, rimBaseUV).g;
            float rimB = rimLightTex.sample(rimSampler, rimBaseUV + rimDispDir).b;
            bgColor += float3(rimR, rimG, rimB);
        }

        // ── Glimmer Glow ──
        if (glass.glowIntensity > 0.001) {
            float2 glowUV = in.uv * glass.cropUVScale + glass.cropUVOffset;
            float3 glowColor = glowTex.sample(s, glowUV).rgb;
            float glowStr = glass.glowIntensity;

            if (glass.glowBlendMode == 0) {
                // Screen blend: A + B - A*B
                bgColor = bgColor + glowColor * glowStr - bgColor * glowColor * glowStr;
            } else if (glass.glowBlendMode == 1) {
                // Additive
                bgColor += glowColor * glowStr;
            } else {
                // Soft light
                float3 a = bgColor;
                float3 b = glowColor * glowStr;
                bgColor = (1.0 - 2.0 * b) * a * a + 2.0 * b * a;
            }
        }

        bgColor = clamp(bgColor, 0.0, 1.0);

        float glassAlpha = shapeMask * in.opacity;
        // Screen-blend the brilliance flare onto the glass surface.
        float3 glassSurface = bgColor * glassAlpha;
        float3 flareContrib = flare * in.opacity;
        float3 result = glassSurface + flareContrib - glassSurface * flareContrib;
        float flareMax = max(flare.r, max(flare.g, flare.b));
        float alpha = min(glassAlpha + flareMax * in.opacity, 1.0);

        return float4(result, alpha);
    }

    // ════════════════════════════════════════════════════════════════════
    // MARK: - Unified Goo Rendering (group-glass-v2)
    // ════════════════════════════════════════════════════════════════════
    //
    // One render pass per goo group. One MTKView per group.
    // The drawable covers the group bounding box exactly, so UV space is
    // self-consistent: localPos ∈ [-groupSize/2, +groupSize/2] maps directly
    // to texture UV ∈ [0, 1]. No coordinate space mismatch. No capture size
    // division in refractUV. No seams.

    // ── Smooth-min (polynomial, C1 continuous) ──
    float smin(float a, float b, float k) {
        float h = max(k - abs(a - b), 0.0) / k;
        return min(a, b) - h * h * k * 0.25;
    }

    // ── Goo Shape Descriptor ──
    struct ConfluenceShapeDescriptor {
        float2 position;              // Center relative to group center (canvas pts)
        float2 halfSize;
        float4 cornerRadii;           // TL, TR, BR, BL
        float  rotation;
        uint   shapeType;             // 0=rect, 1=ellipse, 2=polygon, 3=star
        uint   sides;
        float  innerRadius;
        float  outerRadius;
        float  polygonBorderRadius;
        float  starInnerBorderRadius;
        float  cornerSmoothing;
        int    sdfTextureIndex;       // -1 = analytic only
        float2 sdfMaskPadding;
        float  sdfTexelToPoint;
        // Per-shape material
        float4 tintColor;
        float  tintOpacity;
        float  refractionStrength;
        float  frost;
        float  dispersion;
        float  depthScale;
        float  lightAngle;
        float  lightIntensity;
        float  lightBanding;
        float  edgeWidth;
        float  splayStrength;
    };

    // ── Goo Group Uniforms ──
    struct ConfluenceGroupUniforms {
        uint   glassVariant;
        float2 size;                  // Group quad size (canvas pts) = drawable size
        float  splayStrength;
        float  canvasZoom;
        float2 cursorWorldPos;
        float  cursorActive;
        uint   resonanceEnabled;
        uint   shapeCount;
        float  smoothK;
        float  tiltY;                     // Forward/back tilt (−0.5…+0.5) for near/far flare modulation
        // Capture metadata (set by host: blurPadding=0, captureHalfExtent=size*0.5, captureOffset=0)
        float2 blurPadding;
        float2 captureHalfExtent;
        float2 captureOffset;
        uint   luminanceEnabled;
        uint   brillianceCount;
        float2 brillianceSource0;
        float2 brillianceSource1;
        float2 brillianceSource2;
        float2 brillianceSource3;
        float brillianceMargin;
        float3 brillianceTint0;         // Per-light adaptive tint color (white if unused)
        float3 brillianceTint1;
        float3 brillianceTint2;
        float3 brillianceTint3;
        float glowIntensity;             // 0 = off, 0-1+ = glow strength
        uint glowBlendMode;              // 0 = screen, 1 = additive, 2 = soft light
        uint appearanceMode;             // 0 = base (no contrast), 1 = light, 2 = dark
    };

    // ── Per-shape analytic SDF ──
    float sdGooShape(float2 worldPos, constant ConfluenceShapeDescriptor &shape) {
        float2 relPos = worldPos - shape.position;
        float cosR = cos(-shape.rotation), sinR = sin(-shape.rotation);
        float2 localPos = float2(relPos.x*cosR - relPos.y*sinR, relPos.x*sinR + relPos.y*cosR);
        float2 hs = shape.halfSize;
        float4 radii = min(shape.cornerRadii, float4(min(hs.x, hs.y)));
        float minHalf = min(hs.x, hs.y);
        switch (shape.shapeType) {
            case 1: return sdEllipse(localPos, hs);
            case 2: {
                float2 scale = hs / max(minHalf, 1e-4);
                return sdPolygon(localPos / scale, shape.sides, minHalf, shape.polygonBorderRadius);
            }
            case 3: {
                float2 scale = hs / max(minHalf, 1e-4);
                float outerR = minHalf * shape.outerRadius;
                float innerR = outerR * shape.innerRadius;
                return sdStar(localPos / scale, shape.sides, innerR, outerR,
                              shape.polygonBorderRadius, shape.starInnerBorderRadius);
            }
            default: return sdSuperellipseRect(localPos, hs, radii, shape.cornerSmoothing);
        }
    }

    // ── Sample one of the 8 per-shape SDF textures by index ──
    float sampleGooSDF(int idx, float2 uv,
                       texture2d<float> sdf0, texture2d<float> sdf1,
                       texture2d<float> sdf2, texture2d<float> sdf3,
                       texture2d<float> sdf4, texture2d<float> sdf5,
                       texture2d<float> sdf6, texture2d<float> sdf7) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);
        switch (idx) {
            case 0: return sdf0.sample(s, uv).r;
            case 1: return sdf1.sample(s, uv).r;
            case 2: return sdf2.sample(s, uv).r;
            case 3: return sdf3.sample(s, uv).r;
            case 4: return sdf4.sample(s, uv).r;
            case 5: return sdf5.sample(s, uv).r;
            case 6: return sdf6.sample(s, uv).r;
            case 7: return sdf7.sample(s, uv).r;
            default: return 1e10;
        }
    }

    // Samples the .a (heat field) channel — smooth Jacobi scalar, no Voronoi discontinuities.
    // Use for gradient direction computation; use sampleGooSDF (.r) for distance/smin only.
    float sampleGooHeight(int idx, float2 uv,
                          texture2d<float> sdf0, texture2d<float> sdf1,
                          texture2d<float> sdf2, texture2d<float> sdf3,
                          texture2d<float> sdf4, texture2d<float> sdf5,
                          texture2d<float> sdf6, texture2d<float> sdf7) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);
        switch (idx) {
            case 0: return sdf0.sample(s, uv).a;
            case 1: return sdf1.sample(s, uv).a;
            case 2: return sdf2.sample(s, uv).a;
            case 3: return sdf3.sample(s, uv).a;
            case 4: return sdf4.sample(s, uv).a;
            case 5: return sdf5.sample(s, uv).a;
            case 6: return sdf6.sample(s, uv).a;
            case 7: return sdf7.sample(s, uv).a;
            default: return 0.0;
        }
    }

    // ── Get SDF texture dimensions by index ──
    float2 gooSDFTexSize(int idx,
                         texture2d<float> sdf0, texture2d<float> sdf1,
                         texture2d<float> sdf2, texture2d<float> sdf3,
                         texture2d<float> sdf4, texture2d<float> sdf5,
                         texture2d<float> sdf6, texture2d<float> sdf7) {
        switch (idx) {
            case 0: return float2(sdf0.get_width(), sdf0.get_height());
            case 1: return float2(sdf1.get_width(), sdf1.get_height());
            case 2: return float2(sdf2.get_width(), sdf2.get_height());
            case 3: return float2(sdf3.get_width(), sdf3.get_height());
            case 4: return float2(sdf4.get_width(), sdf4.get_height());
            case 5: return float2(sdf5.get_width(), sdf5.get_height());
            case 6: return float2(sdf6.get_width(), sdf6.get_height());
            case 7: return float2(sdf7.get_width(), sdf7.get_height());
            default: return float2(1.0);
        }
    }

    // ── Per-shape SDF: analytic for primitives, texture-based for custom paths ──
    float sdGooShapeSDF(float2 worldPos, constant ConfluenceShapeDescriptor &shape,
                        texture2d<float> sdf0, texture2d<float> sdf1,
                        texture2d<float> sdf2, texture2d<float> sdf3,
                        texture2d<float> sdf4, texture2d<float> sdf5,
                        texture2d<float> sdf6, texture2d<float> sdf7) {
        if (shape.sdfTextureIndex < 0) {
            return sdGooShape(worldPos, shape);
        }

        // Map world position → shape-local UV → padded SDF texture UV
        float2 relPos = worldPos - shape.position;
        // Rotate into shape's local frame (undo shape rotation)
        float cosR = cos(-shape.rotation), sinR = sin(-shape.rotation);
        float2 localPos = float2(relPos.x*cosR - relPos.y*sinR, relPos.x*sinR + relPos.y*cosR);
        float2 shapeSize = max(shape.halfSize * 2.0, float2(1e-4));
        float2 shapeUV = localPos / shapeSize + 0.5;
        float2 pad = shape.sdfMaskPadding;
        float2 range = max(1.0 - 2.0 * pad, float2(1e-4));
        float2 sdfUV = clamp(pad + shapeUV * range, float2(0.0), float2(1.0));

        float rawSDF = sampleGooSDF(shape.sdfTextureIndex, sdfUV,
                                    sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        float texDist = rawSDF * shape.sdfTexelToPoint;

        // The JFA distance field develops rectangular isolines near the texture
        // edges because the flood can't propagate beyond the texture boundary.
        // Fix: also compute the analytic bounding-rect SDF. The rect always
        // underestimates interior depth / overestimates exterior distance for
        // any shape contained within it. Taking the max uses the texture's
        // accurate shape detail near the boundary while the rect dominates far
        // away — eliminating rectangular ghost artifacts.
        float rectDist = sdSuperellipseRect(localPos, shape.halfSize,
                                            min(shape.cornerRadii, float4(min(shape.halfSize.x, shape.halfSize.y))),
                                            shape.cornerSmoothing);
        float dist = max(texDist, rectDist);

        if (dist > 0.0) dist = min(dist, 80.0);
        return dist;
    }

    // ── Miniature SDF sample for ghost orbs ──
    // Creates a miniature copy of the shape at the ghost position. The pixel
    // offset from the ghost center is scaled up to shape-space, the SDF is
    // queried there, and the result is scaled back down to ghost-space.
    //
    // For analytic shapes: queries sdGooShape (zero texture cost).
    // For texture shapes (text, custom paths): queries sdGooShapeSDF which
    // does a single SDF texture sample. Now that text nodes have clean glyph
    // masks (no bounding-box artifacts), UV clamping at the texture edge
    // returns large positive SDF values that naturally fade the ghost to zero.
    float sampleGhostSDF(float2 pixelPos, float2 ghostCenter, float radius,
                         constant ConfluenceShapeDescriptor &shape,
                         texture2d<float> sdf0, texture2d<float> sdf1,
                         texture2d<float> sdf2, texture2d<float> sdf3,
                         texture2d<float> sdf4, texture2d<float> sdf5,
                         texture2d<float> sdf6, texture2d<float> sdf7) {
        float mh = max(min(shape.halfSize.x, shape.halfSize.y), 1.0);
        float scale = mh / max(radius, 1e-4);
        float2 scaledOffset = (pixelPos - ghostCenter) * scale;
        float2 queryPos = shape.position + scaledOffset;

        float d;
        if (shape.sdfTextureIndex < 0) {
            d = sdGooShape(queryPos, shape);
        } else {
            d = sdGooShapeSDF(queryPos, shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        }
        return d / scale;
    }

    // ── Shape-conforming ghost orb for goo shapes ──
    // Creates a miniature shape at the ghost position. 1 SDF sample per call.
    float gooShapeGhost(float2 pixelPos, float2 ghostCenter, float radius,
                        constant ConfluenceShapeDescriptor &shape,
                        texture2d<float> sdf0, texture2d<float> sdf1,
                        texture2d<float> sdf2, texture2d<float> sdf3,
                        texture2d<float> sdf4, texture2d<float> sdf5,
                        texture2d<float> sdf6, texture2d<float> sdf7) {
        float d = sampleGhostSDF(pixelPos, ghostCenter, radius, shape,
                                 sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        // Frost widens the falloff so ghosts blur out like the frosted backdrop.
        // Custom paths use a tighter base to prevent fog bleed at the texture edge.
        bool isSdfShape = shape.sdfTextureIndex >= 0;
        float frostBlur = 1.0 + clamp(shape.frost, 0.0, 1.0) * 6.0;
        float baseFalloff = isSdfShape ? 0.05 : 0.15;
        float maxFalloff = isSdfShape ? 0.12 : 1e6;
        float falloff = min(radius * baseFalloff * frostBlur, radius * maxFalloff);
        float dOut = max(d, 0.0);
        return exp(-(dOut * dOut) / (falloff * falloff));
    }

    // ── Shape-conforming filled ring ghost for goo shapes ──
    // Solid inside the shape, soft Gaussian fade outside.
    float gooShapeRing(float2 pixelPos, float2 ghostCenter, float radius, float ringWidth,
                       constant ConfluenceShapeDescriptor &shape,
                       texture2d<float> sdf0, texture2d<float> sdf1,
                       texture2d<float> sdf2, texture2d<float> sdf3,
                       texture2d<float> sdf4, texture2d<float> sdf5,
                       texture2d<float> sdf6, texture2d<float> sdf7) {
        float mh = max(min(shape.halfSize.x, shape.halfSize.y), 1.0);
        float scale = mh / max(radius, 1e-4);
        float2 scaledOffset = (pixelPos - ghostCenter) * scale;
        float2 queryPos = shape.position + scaledOffset;

        float d;
        if (shape.sdfTextureIndex < 0) {
            d = sdGooShape(queryPos, shape);
        } else {
            d = sdGooShapeSDF(queryPos, shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        }
        bool isSdfShape = shape.sdfTextureIndex >= 0;
        float frostBlur = 1.0 + clamp(shape.frost, 0.0, 1.0) * 6.0;
        float baseRingFalloff = isSdfShape ? 0.02 : 0.06;
        float maxRingFalloff = isSdfShape ? 0.05 : 1e6;
        float ringFalloff = min(mh * baseRingFalloff * frostBlur, mh * maxRingFalloff);
        float dOut = max(d, 0.0);
        return exp(-(dOut * dOut) / (ringFalloff * ringFalloff));
    }

    // ── Per-shape geometric refraction direction + gradWeight ──
    // Mirrors the standalone glass per-shape normal logic exactly so polygons/stars
    // get the same medial-axis suppression as the non-goo path.
    struct GooShapeRefract {
        float2 inwardDir;
        float  gradWeight;
    };

    GooShapeRefract gooShapeRefractDir(
        float2 worldPos,
        constant ConfluenceShapeDescriptor &shape,
        float nd, float edgeExp,
        texture2d<float> sdf0, texture2d<float> sdf1,
        texture2d<float> sdf2, texture2d<float> sdf3,
        texture2d<float> sdf4, texture2d<float> sdf5,
        texture2d<float> sdf6, texture2d<float> sdf7
    ) {
        GooShapeRefract result;

        // Transform to shape-local (rotated) space
        float2 relPos = worldPos - shape.position;
        float cosR = cos(-shape.rotation), sinR = sin(-shape.rotation);
        float2 localPos = float2(relPos.x*cosR - relPos.y*sinR, relPos.x*sinR + relPos.y*cosR);
        float2 hs = shape.halfSize;
        float minHalf = min(hs.x, hs.y);

        // inwardDir will be computed in shape-local space, then rotated to world space at the end
        float2 inwardDirLocal = float2(0.0);

        if (shape.shapeType == 2) {
            // ── Polygon: finite-difference on the analytic polygon SDF ──
            // Same approach as the rect branch — the polygon SDF gradient
            // naturally fades near vertices (just like rect corners), giving
            // seamless medial-axis suppression without manual edge iteration.
            float delta = max(length(dfdx(localPos)), 0.5) * 2.0;
            float dR = sdGooShapeSDF(worldPos + float2( delta, 0.0), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dL = sdGooShapeSDF(worldPos + float2(-delta, 0.0), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dU = sdGooShapeSDF(worldPos + float2(0.0, -delta), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dD = sdGooShapeSDF(worldPos + float2(0.0,  delta), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float2 sdfGrad = float2(dR - dL, dD - dU);
            float sdfGradMag = length(sdfGrad);
            result.gradWeight = smoothstep(0.01, 0.12, sdfGradMag / max(delta, 1e-4));
            float2 edgeDirWorld = (sdfGradMag > 1e-5) ? (-sdfGrad / sdfGradMag) : float2(0.0);
            float2 centroidDirWorld = -safeNormalize(relPos);
            float blendToCenter = smoothstep(0.15, 0.4, nd);
            result.inwardDir = (sdfGradMag > 1e-5)
                ? normalize(mix(edgeDirWorld, centroidDirWorld, blendToCenter))
                : centroidDirWorld;
            return result;  // already in world space

        } else if (shape.shapeType == 3) {
            // ── Star: warp-field gradient (same as standalone) ──
            float2 starScale = hs / max(minHalf, 1e-4);
            float2 starPos = localPos / starScale;
            float outerR = minHalf * shape.outerRadius;
            float innerR = outerR * shape.innerRadius;
            float midR = (outerR + innerR) * 0.5;
            float ampR = (outerR - innerR) * 0.5;
            float n = float(max(shape.sides, 3u));

            float delta = max(length(dfdx(starPos)), 0.5) * 2.0;
            float2 sp1 = starPos + float2( delta, 0.0);
            float2 sp2 = starPos + float2(-delta, 0.0);
            float2 sp3 = starPos + float2(0.0, -delta);
            float2 sp4 = starPos + float2(0.0,  delta);
            float wr1 = length(sp1) / max(midR + ampR * cos(n * atan2(sp1.x, sp1.y)), 1e-4);
            float wr2 = length(sp2) / max(midR + ampR * cos(n * atan2(sp2.x, sp2.y)), 1e-4);
            float wr3 = length(sp3) / max(midR + ampR * cos(n * atan2(sp3.x, sp3.y)), 1e-4);
            float wr4 = length(sp4) / max(midR + ampR * cos(n * atan2(sp4.x, sp4.y)), 1e-4);
            float2 warpGrad = float2(wr1 - wr2, wr4 - wr3);
            float warpGradMag = length(warpGrad);

            result.gradWeight = smoothstep(0.01, 0.12, warpGradMag);
            inwardDirLocal = (warpGradMag > 1e-5) ? (-warpGrad / warpGradMag) : float2(0.0);

        } else if (shape.shapeType == 0 && shape.sdfTextureIndex >= 0) {
            // ── Custom path: height-field gradient from SDF texture alpha ──
            // Use .a (smooth Jacobi heat field) not .r (JFA distance) to avoid
            // Voronoi discontinuities that cause ripples and seams at the medial axis.
            // Matches the standalone glass path (fragment_glass_composite) exactly.
            float2 shapeUV = localPos / max(hs * 2.0, float2(1e-4)) + 0.5;
            float2 pad = shape.sdfMaskPadding;
            float2 range = max(1.0 - 2.0 * pad, float2(1e-4));
            float2 sdfUV = clamp(pad + shapeUV * range, float2(0.0), float2(1.0));
            float2 sdfTexSize = gooSDFTexSize(shape.sdfTextureIndex, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float2 texelSize = 1.0 / max(sdfTexSize, float2(1.0));
            float hL = sampleGooHeight(shape.sdfTextureIndex, sdfUV + float2(-texelSize.x, 0.0), sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float hR = sampleGooHeight(shape.sdfTextureIndex, sdfUV + float2( texelSize.x, 0.0), sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float hT = sampleGooHeight(shape.sdfTextureIndex, sdfUV + float2(0.0, -texelSize.y), sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float hB = sampleGooHeight(shape.sdfTextureIndex, sdfUV + float2(0.0,  texelSize.y), sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float2 heightGrad = float2(hR - hL, hB - hT) * 0.5;
            float gradMag = length(heightGrad);
            result.gradWeight = smoothstep(0.005, 0.12, gradMag);
            inwardDirLocal = (gradMag > 1e-5) ? (heightGrad / gradMag) : float2(0.0);

        } else {
            // ── Rounded rect / ellipse: finite-difference on single shape SDF ──
            // sdGooShapeSDF works in world space, so sample there and the gradient
            // comes back in world space — no need to rotate at the end for this branch.
            float delta = max(length(dfdx(localPos)), 0.5) * 2.0;
            float dR = sdGooShapeSDF(worldPos + float2( delta, 0.0), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dL = sdGooShapeSDF(worldPos + float2(-delta, 0.0), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dU = sdGooShapeSDF(worldPos + float2(0.0, -delta), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float dD = sdGooShapeSDF(worldPos + float2(0.0,  delta), shape, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float2 sdfGrad = float2(dR - dL, dD - dU);
            float sdfGradMag = length(sdfGrad);
            result.gradWeight = smoothstep(0.01, 0.12, sdfGradMag / max(delta, 1e-4));
            float2 edgeDirWorld = (sdfGradMag > 1e-5) ? (-sdfGrad / sdfGradMag) : float2(0.0);
            float2 centroidDirWorld = -safeNormalize(relPos);
            float blendToCenter = smoothstep(0.15, 0.4, nd);
            result.inwardDir = (length(edgeDirWorld) > 1e-5)
                ? normalize(mix(edgeDirWorld, centroidDirWorld, blendToCenter))
                : centroidDirWorld;
            return result;  // already in world space
        }

        // Rotate inwardDirLocal from shape-local space back to world (canvas) space.
        // The forward transform was rotation by -shape.rotation, so inverse is +shape.rotation.
        float cosRfwd = cos(shape.rotation), sinRfwd = sin(shape.rotation);
        result.inwardDir = float2(
            inwardDirLocal.x * cosRfwd - inwardDirLocal.y * sinRfwd,
            inwardDirLocal.x * sinRfwd + inwardDirLocal.y * cosRfwd
        );

        return result;
    }

    // ── Smooth-min union ──
    float sdGooUnifiedField(float2 worldPos,
                            constant ConfluenceShapeDescriptor *shapes, uint shapeCount, float k,
                            texture2d<float> sdf0, texture2d<float> sdf1,
                            texture2d<float> sdf2, texture2d<float> sdf3,
                            texture2d<float> sdf4, texture2d<float> sdf5,
                            texture2d<float> sdf6, texture2d<float> sdf7) {
        if (shapeCount == 0) return 1e10;
        float result = sdGooShapeSDF(worldPos, shapes[0], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        for (uint i = 1; i < shapeCount; i++) {
            result = smin(result, sdGooShapeSDF(worldPos, shapes[i], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7), k);
        }
        return result;
    }

    // ── Unified field gradient (finite-difference) ──
    float2 gooUnifiedGradient(float2 worldPos, float delta,
                              constant ConfluenceShapeDescriptor *shapes, uint shapeCount, float k,
                              texture2d<float> sdf0, texture2d<float> sdf1,
                              texture2d<float> sdf2, texture2d<float> sdf3,
                              texture2d<float> sdf4, texture2d<float> sdf5,
                              texture2d<float> sdf6, texture2d<float> sdf7) {
        float dR = sdGooUnifiedField(worldPos + float2( delta, 0.0), shapes, shapeCount, k, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        float dL = sdGooUnifiedField(worldPos + float2(-delta, 0.0), shapes, shapeCount, k, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        float dU = sdGooUnifiedField(worldPos + float2(0.0, -delta), shapes, shapeCount, k, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        float dD = sdGooUnifiedField(worldPos + float2(0.0,  delta), shapes, shapeCount, k, sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        return float2(dR - dL, dD - dU);
    }

    // ── Interior distance ──
    float gooInteriorDistance(float2 worldPos,
                              constant ConfluenceShapeDescriptor *shapes, uint shapeCount, float mergedD,
                              texture2d<float> sdf0, texture2d<float> sdf1,
                              texture2d<float> sdf2, texture2d<float> sdf3,
                              texture2d<float> sdf4, texture2d<float> sdf5,
                              texture2d<float> sdf6, texture2d<float> sdf7) {
        return max(-mergedD, 0.0);
    }

    // ── Per-shape material blending ──
    struct GooBlendedMaterial {
        float4 tintColor;
        float  tintOpacity;
        float  refractionStrength;
        float  frost;
        float  dispersion;
        float  depthScale;
        float  lightAngle;
        float  lightIntensity;
        float  lightBanding;
        float  edgeWidth;
        float  splayStrength;
        float2 blendedCenter;         // Softmax-blended shape center (group-local coords)
        float2 closestShapeCenter;
        int    closestShapeIdx;
        float  closestShapeDist;
        int    secondClosestIdx;
        float  secondClosestDist;
    };

    GooBlendedMaterial gooBlendMaterial(
        constant ConfluenceShapeDescriptor *shapes, uint shapeCount, float k,
        thread float *cachedSDF
    ) {
        GooBlendedMaterial mat;
        mat.tintColor = float4(0); mat.tintOpacity = 0;
        mat.refractionStrength = 0; mat.frost = 0; mat.dispersion = 0;
        mat.depthScale = 0; mat.lightAngle = 0; mat.lightIntensity = 0;
        mat.lightBanding = 0; mat.edgeWidth = 0; mat.splayStrength = 0;
        mat.blendedCenter = float2(0);
        mat.closestShapeCenter = float2(0);
        mat.closestShapeIdx = 0; mat.closestShapeDist = 0.0;
        mat.secondClosestIdx = -1; mat.secondClosestDist = 1e10;

        if (shapeCount == 0) return mat;
        if (shapeCount == 1) {
            mat.tintColor = shapes[0].tintColor; mat.tintOpacity = shapes[0].tintOpacity;
            mat.refractionStrength = shapes[0].refractionStrength; mat.frost = shapes[0].frost;
            mat.dispersion = shapes[0].dispersion; mat.depthScale = shapes[0].depthScale;
            mat.lightAngle = shapes[0].lightAngle; mat.lightIntensity = shapes[0].lightIntensity;
            mat.lightBanding = shapes[0].lightBanding; mat.edgeWidth = shapes[0].edgeWidth;
            mat.splayStrength = shapes[0].splayStrength;
            mat.blendedCenter = shapes[0].position;
            mat.closestShapeCenter = shapes[0].position;
            mat.closestShapeIdx = 0; mat.closestShapeDist = -1e10;
            return mat;
        }

        float closestDist = 1e10, secondDist = 1e10;

        for (uint i = 0; i < shapeCount; i++) {
            float sd = cachedSDF[i];
            if (sd < closestDist) {
                secondDist = closestDist; mat.secondClosestIdx = mat.closestShapeIdx;
                closestDist = sd; mat.closestShapeCenter = shapes[i].position; mat.closestShapeIdx = int(i);
            } else if (sd < secondDist) { secondDist = sd; mat.secondClosestIdx = int(i); }
        }
        mat.closestShapeDist = closestDist; mat.secondClosestDist = secondDist;

        // Softmax over -SDF: pixels deep inside a shape weight it exponentially more.
        // Temperature matches the smin influence radius (k * 0.5) so every material
        // property eases across the full merge zone — same spatial width as the shape
        // blend itself. Deep inside one shape it dominates; at the seam they blend smoothly.
        float temp = max(k * 0.5, 1.0);
        float weights[8];
        float totalWeight = 0.0;
        for (uint i = 0; i < shapeCount; i++) {
            float w = exp(-cachedSDF[i] / temp);
            weights[i] = w; totalWeight += w;
        }

        float invTotal = 1.0 / max(totalWeight, 1e-6);
        for (uint i = 0; i < shapeCount; i++) {
            float w = weights[i] * invTotal;
            mat.tintColor += shapes[i].tintColor * w;
            mat.tintOpacity += shapes[i].tintOpacity * w;
            mat.refractionStrength += shapes[i].refractionStrength * w;
            mat.frost += shapes[i].frost * w;
            mat.dispersion += shapes[i].dispersion * w;
            mat.depthScale += shapes[i].depthScale * w;
            mat.lightAngle += shapes[i].lightAngle * w;
            mat.lightIntensity += shapes[i].lightIntensity * w;
            mat.lightBanding += shapes[i].lightBanding * w;
            mat.edgeWidth += shapes[i].edgeWidth * w;
            mat.splayStrength += shapes[i].splayStrength * w;
            mat.blendedCenter += shapes[i].position * w;
        }
        return mat;
    }

    // ══════════════════════════════════════════════════════════════════
    // MARK: - Goo Fragment Shaders
    // ══════════════════════════════════════════════════════════════════

    // ── Goo Composite ──
    fragment float4 fragment_goo_composite(
        GlassVertexOut in [[stage_in]],
        texture2d<float> blurTex   [[texture(0)]],
        texture2d<float> sharpTex  [[texture(1)]],
        texture2d<float> sdf0 [[texture(2)]],
        texture2d<float> sdf1 [[texture(3)]],
        texture2d<float> sdf2 [[texture(4)]],
        texture2d<float> sdf3 [[texture(5)]],
        texture2d<float> sdf4 [[texture(6)]],
        texture2d<float> sdf5 [[texture(7)]],
        texture2d<float> sdf6 [[texture(8)]],
        texture2d<float> sdf7 [[texture(9)]],
        texture2d<float> glowTex [[texture(10)]],
        constant ConfluenceGroupUniforms &goo [[buffer(0)]],
        constant ConfluenceShapeDescriptor *shapes [[buffer(1)]],
        constant float4x4 &viewProjection [[buffer(2)]]
    ) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);

        // localPos: canvas pts relative to group center
        float2 quadSize = goo.size + float2(goo.brillianceMargin * 2.0);
        float2 localPos = (in.uv - 0.5) * quadSize;

        // Cache per-shape SDF values at localPos once — reused by blend material,
        // refraction, brilliance, and rim light loops to avoid redundant evaluations.
        float cachedSDF[8];
        for (uint ci = 0; ci < goo.shapeCount; ci++) {
            cachedSDF[ci] = sdGooShapeSDF(localPos, shapes[ci], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        }

        // Compute unified field (smin) from cached values instead of re-evaluating per shape.
        float d = (goo.shapeCount > 0) ? cachedSDF[0] : 1e10;
        for (uint ci = 1; ci < goo.shapeCount; ci++) {
            d = smin(d, cachedSDF[ci], goo.smoothK);
        }

        float2 dpdx_lp = dfdx(localPos), dpdy_lp = dfdy(localPos);
        float pixelSize = length(float2(length(dpdx_lp), length(dpdy_lp))) * 0.7071;
        float aa = max(pixelSize, 0.75);
        float shapeMask = 1.0 - smoothstep(-aa, aa, d);

        float3 flare = float3(0.0);
        if (shapeMask < 0.001 && goo.brillianceCount == 0) return float4(0.0);

        float interiorDistance = gooInteriorDistance(localPos, shapes, goo.shapeCount, d,
                                                     sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        GooBlendedMaterial mat = gooBlendMaterial(shapes, goo.shapeCount, goo.smoothK, cachedSDF);

        float maxShapeMinHalf = 0.0;
        for (uint si2 = 0; si2 < goo.shapeCount; si2++) {
            maxShapeMinHalf = max(maxShapeMinHalf, min(shapes[si2].halfSize.x, shapes[si2].halfSize.y));
        }
        float minHalf = max(maxShapeMinHalf, 1.0);
        float maxInteriorDist = minHalf * 0.5;

        float falloffFrac = mix(0.01, 0.60, mat.edgeWidth);
        float falloffPx = max(6.0, minHalf * falloffFrac);

        // Use the unified SDF interior distance for all edge-dependent quantities so
        // the bridge zone is treated as one continuous surface — no seam between shapes.
        float unifiedInteriorDist = max(-d, 0.0);

        // For custom-path shapes, blend SDF distance with height field to avoid
        // Voronoi medial-axis artifacts in interior distance (matches standalone path).
        int closestIdx = mat.closestShapeIdx;
        bool closestIsCustom = (shapes[closestIdx].shapeType == 0 && shapes[closestIdx].sdfTextureIndex >= 0);
        if (closestIsCustom) {
            constant ConfluenceShapeDescriptor &cs = shapes[closestIdx];
            float2 relPos = localPos - cs.position;
            float cosR = cos(-cs.rotation), sinR = sin(-cs.rotation);
            float2 csLocal = float2(relPos.x*cosR - relPos.y*sinR, relPos.x*sinR + relPos.y*cosR);
            float2 shapeUV = csLocal / max(cs.halfSize * 2.0, float2(1e-4)) + 0.5;
            float2 pad = cs.sdfMaskPadding;
            float2 range = max(1.0 - 2.0 * pad, float2(1e-4));
            float2 sdfUV = clamp(pad + shapeUV * range, float2(0.0), float2(1.0));
            float heightVal = sampleGooHeight(cs.sdfTextureIndex, sdfUV,
                                              sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
            float heightInterior = heightVal * cs.sdfTexelToPoint;
            float sdfInterior = unifiedInteriorDist;
            float edgeBlend = smoothstep(3.0, 6.0, sdfInterior);
            unifiedInteriorDist = mix(sdfInterior, heightInterior, edgeBlend);
        }

        float edgeProximity = 1.0 - smoothstep(0.0, falloffPx, unifiedInteriorDist);
        float lensProfile = closestIsCustom
            ? (1.0 - clamp(unifiedInteriorDist / max(maxInteriorDist, 1.0), 0.0, 1.0))
            : edgeProximity;

        float edgeExp = mix(6.0, 2.0, mat.edgeWidth);

        float nd = clamp(unifiedInteriorDist / max(maxInteriorDist, 1.0), 0.0, 1.0);
        // Custom paths use the standalone's edgeWeight: smoothstep-shaped depth
        // fed into a softer exponent. This matches fragment_glass_composite exactly.
        float edgeWeight;
        if (closestIsCustom) {
            float sdfDepth = 1.0 - clamp(edgeProximity, 0.0, 1.0);
            edgeWeight = refractionFalloff(sdfDepth, max(1.0, edgeExp - 0.5));
        } else {
            edgeWeight = refractionFalloff(nd, edgeExp);
        }

        // ── Per-shape geometric refraction direction — softmax blended ──
        // Loop over all shapes with the same temperature as gooBlendMaterial so the
        // refraction direction interpolates smoothly across bridge zones. Using only
        // the closest shape caused a discrete flip at the bridge midpoint → visible seam.
        float delta = max(length(dfdx(localPos)), 0.5) * 2.0;
        float2 blendedInwardDir = float2(0.0);
        float  blendedGradWeight = 0.0;
        float  refractBlendTotal = 0.0;
        float  refractBlendTemp = max(goo.smoothK, 1.0);
        for (uint ri = 0; ri < goo.shapeCount; ri++) {
            float rsd = cachedSDF[ri];
            float rw = exp(-rsd / refractBlendTemp);
            GooShapeRefract sr = gooShapeRefractDir(
                localPos, shapes[ri], nd, edgeExp,
                sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7
            );
            blendedInwardDir   += sr.inwardDir   * rw;
            blendedGradWeight  += sr.gradWeight   * rw;
            refractBlendTotal  += rw;
        }
        float invRefractW = 1.0 / max(refractBlendTotal, 1e-6);
        blendedInwardDir  *= invRefractW;
        blendedGradWeight *= invRefractW;
        float blendedDirLen = length(blendedInwardDir);
        float2 finalInwardDir = blendedDirLen > 1e-5 ? blendedInwardDir / blendedDirLen : float2(0.0);

        // surfaceNormal for Fresnel/rim: unified field gradient (correct for merged shapes)
        float2 sdfGrad = gooUnifiedGradient(localPos, delta, shapes, goo.shapeCount, goo.smoothK,
                                            sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
        float sdfGradMag = length(sdfGrad);
        float2 surfaceNormal = (sdfGradMag > 1e-5) ? (sdfGrad / sdfGradMag) : float2(0.0, -1.0);

        // ── Refraction UV ──
        float variantRefractionScale = (goo.glassVariant == 1) ? 0.65 : 1.0;
        float refractionMaxDist = min(maxInteriorDist, 30.0);
        float2 refractUV = finalInwardDir * edgeWeight * blendedGradWeight
                         * mat.refractionStrength * variantRefractionScale * refractionMaxDist;

        float lensBlend = lensProfile * lensProfile;
        refractUV *= mix(1.0, lensBlend, max(mat.edgeWidth, 0.0));

        float frostAmount = clamp(mat.frost, 0.0, 1.0);

        // ── Background UV ──
        // Project this fragment's world position through the viewProjection matrix
        // to get its exact screen UV into the full-viewport background texture.
        // The sampler uses clamp_to_edge, so UVs past [0,1] repeat the edge pixel.
        float4 clip = viewProjection * float4(in.worldPos, 0.0, 1.0);
        float2 baseUV = float2(clip.x * 0.5 + 0.5, -clip.y * 0.5 + 0.5);

        // texCenter: softmax-blended shape center projected to screen UV.
        // Smoothly tracks whichever shape dominates at each pixel — no hard
        // transitions like closestIdx, but naturally follows stray shapes.
        float2 groupCenterWorld = in.worldPos - localPos;
        float2 blendedWorld = groupCenterWorld + mat.blendedCenter;
        float4 centerClip = viewProjection * float4(blendedWorld, 0.0, 1.0);
        float2 texCenter = float2(centerClip.x * 0.5 + 0.5, -centerClip.y * 0.5 + 0.5);

        // Shape-to-viewport ratio: shape half-extent in viewport UV, zoom-independent.
        // viewProjection[0][0] = 2*zoom/viewportW, so we divide out the zoom to keep
        // all effects constant across zoom levels (matching standalone where paddedRange
        // is zoom-independent).
        float2 closestHalf = shapes[closestIdx].halfSize;
        float zoomInv = 1.0 / max(goo.canvasZoom, 0.001);
        float2 shapeToViewport = closestHalf * float2(abs(viewProjection[0][0]),
                                                       abs(viewProjection[1][1])) * zoomInv;

        // Splay — work in shape-normalized space so the barrel distortion is
        // zoom-independent.  Normalize splayOff by shapeToViewport (shape half-extent
        // in viewport UV), apply the r² distortion in that space, then convert back.
        // This matches the standalone path where everything is in capture-texture UV.
        float splayStrength = abs(mat.splayStrength);
        float2 splayOff = baseUV - texCenter;
        float2 normOff = splayOff / max(shapeToViewport, float2(1e-5));
        float r = length(normOff);
        float2 splayDisp = normOff * splayStrength * r * r * shapeToViewport;
        baseUV += splayDisp;

        // refractUV is in "shape-fraction" units (direction × maxInteriorDist ×
        // refractionStrength). The standalone path applies `refractUV *= paddedRange`
        // where paddedRange ≈ 0.8, but there the background texture covers just the
        // shape's capture region. Here the background is the full viewport, so we need
        // to scale by (shape extent / viewport extent) to get equivalent displacement.
        float2 scaledRefractUV = refractUV * shapeToViewport;

        float depthScale = mat.depthScale;

        float2 uvMin = float2(0.001), uvMax = float2(0.999);
        float maxDisp = mat.dispersion * 3.0;

        // Clamp refraction to avoid sampling outside texture bounds
        {
            float2 refractedTest = baseUV + scaledRefractUV * (1.0 + maxDisp);
            float2 overshootLo = uvMin - refractedTest;
            float2 overshootHi = refractedTest - uvMax;
            float overshoot = max(max(overshootLo.x, overshootLo.y), max(overshootHi.x, overshootHi.y));
            if (overshoot > 0.0) scaledRefractUV *= 1.0 / (1.0 + overshoot * 4.0);
        }

        // Edge UV fade — smoothly reduce refraction near texture boundaries
        {
            float2 refractedUV = baseUV + scaledRefractUV;
            float2 dMin = refractedUV - (uvMin + 0.01);
            float2 dMax = (uvMax - 0.01) - refractedUV;
            float2 margin = float2(0.05);
            float fadeX = smoothstep(0.0, margin.x, min(dMin.x, dMax.x));
            float fadeY = smoothstep(0.0, margin.y, min(dMin.y, dMax.y));
            scaledRefractUV *= fadeX * fadeY;
        }

        // Chromatic aberration
        float2 normalAber = surfaceNormal * mat.dispersion * 0.15;
        float2 radDir = safeNormalize(baseUV - texCenter);
        float radDist = length((baseUV - texCenter) * 2.0);
        float2 radAber = radDir * mix(0.3, 1.0, clamp(radDist, 0.0, 1.0)) * mat.dispersion * 0.15;
        float normalInfl = smoothstep(0.0, falloffPx * 0.5, interiorDistance);
        float2 aberDir = mix(normalAber, radAber, normalInfl);

        // Spectral sampling — fast path when dispersion is off.
        float3 bgSampled;
        if (mat.dispersion < 0.001) {
            float2 zoomedUV = clamp(texCenter + (baseUV + scaledRefractUV - texCenter) / depthScale, uvMin, uvMax);
            float3 bl = blurTex.sample(s, zoomedUV).rgb;
            float3 sh = sharpTex.sample(s, zoomedUV).rgb;
            bgSampled = mix(sh, bl, frostAmount);
        } else {
            float3 spectralSum = float3(0.0), wSum = float3(0.0);
            for (int band = 0; band < 7; band++) {
                float t = float(band) / 6.0;
                float offset = t * 2.0 - 1.0;
                float2 bandUV = baseUV + scaledRefractUV + aberDir * offset;
                float2 zoomedUV = clamp(texCenter + (bandUV - texCenter) / depthScale, uvMin, uvMax);
                float3 bl = blurTex.sample(s, zoomedUV).rgb;
                float3 sh = sharpTex.sample(s, zoomedUV).rgb;
                float3 samp = mix(sh, bl, frostAmount);
                float3 w = spectralW[band];
                spectralSum += samp * w; wSum += w;
            }
            bgSampled = spectralSum / max(wSum, float3(1e-4));
        }

        // Fresnel reflection
        float fresnelWidth = clamp(minHalf * 0.08, 4.0, 20.0);
        float fresnelZone = 1.0 - smoothstep(0.0, fresnelWidth, interiorDistance);
        float tilt = clamp(fresnelZone, 0.0, 0.999);
        float cosTheta = sqrt(1.0 - tilt * tilt);
        float F0 = 0.04;
        float oneMinusCos = 1.0 - cosTheta;
        float omc2 = oneMinusCos * oneMinusCos;
        float fresnel = F0 + (1.0 - F0) * (omc2 * omc2 * oneMinusCos);
        fresnel *= clamp(mat.refractionStrength * 8.0, 0.0, 0.5);

        float3 reflColor = float3(0.0);
        if (fresnel > 0.001) {
            float2 uvRefl = clamp(texCenter + (baseUV - scaledRefractUV * 1.5 - texCenter) / depthScale, uvMin, uvMax);
            float3 rBl = blurTex.sample(s, uvRefl).rgb;
            float3 rSh = sharpTex.sample(s, uvRefl).rgb;
            reflColor = mix(rSh, rBl, frostAmount);
        }
        float3 bgColor = mix(bgSampled, reflColor, fresnel);

        // Frost shimmer
        if (frostAmount > 0.01) {
            float shimmerEdge = exp(-interiorDistance*interiorDistance / (max(minHalf*0.1, 4.0)*max(minHalf*0.1, 4.0)));
            bgColor += float3(shimmerEdge * shimmerEdge * frostAmount * mat.lightIntensity * 0.12);
        }

        // Tint / Resonance
        float3 resonanceTint = float3(1.0);
        if (goo.resonanceEnabled != 0) {
            // Compute fresh tint from background probes. The goo path renders all
            // merged shapes in one pass so the tint is inherently stable per-pixel —
            // no temporal anchor needed (that's only for the standalone path).
            float2 probeSpan = float2(0.3);
            float3 samples[5];
            samples[0] = blurTex.sample(s, texCenter).rgb;
            samples[1] = blurTex.sample(s, texCenter + float2( probeSpan.x, 0)).rgb;
            samples[2] = blurTex.sample(s, texCenter + float2(-probeSpan.x, 0)).rgb;
            samples[3] = blurTex.sample(s, texCenter + float2(0,  probeSpan.y)).rgb;
            samples[4] = blurTex.sample(s, texCenter + float2(0, -probeSpan.y)).rgb;
            float avgLum = 0.0;
            float3 darkest = float3(1.0), lightest = float3(0.0);
            for (int i = 0; i < 5; i++) {
                float lum = dot(samples[i], float3(0.2126, 0.7152, 0.0722));
                avgLum += lum;
                darkest = min(darkest, samples[i]);
                lightest = max(lightest, samples[i]);
            }
            avgLum /= 5.0;
            resonanceTint = mix(lightest, darkest, avgLum);
            if (mat.tintOpacity > 0.001) resonanceTint = mix(resonanceTint, mat.tintColor.rgb, 0.5);
            float resOpacity = (mat.tintOpacity > 0.001) ? mat.tintOpacity : 0.15;
            bgColor = mix(bgColor, resonanceTint, resOpacity);
        } else if (mat.tintOpacity > 0.001) {
            bgColor = mix(bgColor, mat.tintColor.rgb, mat.tintOpacity);
        }

        // ── Inner light/shadow (luminance-driven) ─────────────────────
        if (goo.luminanceEnabled) {
            float bgLum = dot(bgSampled, float3(0.2126, 0.7152, 0.0722));

            float lightAmount = smoothstep(0.4, 0.6, bgLum);
            float shadowAmount = smoothstep(0.5, 0.3, bgLum);

            bgColor += float3(1.0, 0.98, 0.95) * lightAmount * 0.15;
            bgColor *= 1.0 - shadowAmount * 0.12;
        }

        // ── Brilliance (user-specified light sources → lens flare) ────
        // Per-shape flares blended via softmax weights so each shape maintains
        // its own flare until shapes conjoin, then they merge smoothly.
        // Modulated by depth (ghost size), frost (diffusion), tint/resonance (color).
        if (goo.brillianceCount > 0) {
            float2 sources[4] = {
                goo.brillianceSource0, goo.brillianceSource1,
                goo.brillianceSource2, goo.brillianceSource3
            };

            // Property modulation (from blended material)
            float gooFrostAmount = clamp(mat.frost, 0.0, 1.0);
            float depthGhostScale = sqrt(max(mat.depthScale, 0.5));
            float frostShrink = 1.0 - gooFrostAmount * 0.5;
            float frostDim = 1.0 - gooFrostAmount * 0.6;
            float ghostScale = depthGhostScale * frostShrink;

            // Per-light tint arrays for adaptive canvas sampling
            float3 bTints[4] = {
                goo.brillianceTint0, goo.brillianceTint1,
                goo.brillianceTint2, goo.brillianceTint3
            };

            flare = float3(0.0);
            float blendTemp = max(goo.smoothK * 0.5, 1.0);
            float flareWeightTotal = 0.0;

            for (uint sh = 0; sh < goo.shapeCount; sh++) {
                float shSdf = cachedSDF[sh];
                float shWeight = exp(-shSdf / blendTemp);

                float2 shCenter = groupCenterWorld + shapes[sh].position;
                float shMh = max(min(shapes[sh].halfSize.x, shapes[sh].halfSize.y), 1.0);
                float shDispersion = shapes[sh].dispersion;
                float3 shFlare = float3(0.0);

                for (uint si = 0; si < goo.brillianceCount && si < 4; si++) {
                    float2 lightLocal = sources[si] - shCenter;
                    float lpLen = length(lightLocal);
                    float2 flareAxis = lpLen > 0.001 ? lightLocal / lpLen : float2(0.0, -1.0);

                    // Collapse: ghosts shrink inward as light approaches center
                    float collapseFactor = smoothstep(0.0, shMh * 0.3, lpLen);

                    float proxFade = 1.0 - smoothstep(shMh * 1.0, shMh * 6.0, lpLen);
                    if (proxFade < 0.001) continue;

                    // Per-light tint: explicit tint overrides resonance overrides canvas sampling
                    float3 perLightTint;
                    float perLightTintStr;
                    if (mat.tintOpacity > 0.001) {
                        perLightTint = mat.tintColor.rgb;
                        perLightTintStr = mat.tintOpacity * 0.5;
                    } else if (goo.resonanceEnabled != 0) {
                        perLightTint = resonanceTint;
                        perLightTintStr = 0.5;
                    } else {
                        perLightTint = bTints[si];
                        perLightTintStr = 0.5;
                    }
                    float3 flareTintMul = mix(float3(1.0), perLightTint, perLightTintStr);

                    // Spatial early-out cutoff for ghost SDF samples.
                    bool isSdfShape = shapes[sh].sdfTextureIndex >= 0;
                    float maxGhostFalloff = isSdfShape ? (shMh * 0.36) : (shMh * 3.15);
                    bool chromaticGhosts = shDispersion >= 0.001;

                    // Ghost 1: close to center on opposite side
                    float2 g1 = shapes[sh].position + flareAxis * (-0.35 * shMh * ghostScale * collapseFactor);
                    float r1 = shMh * 0.14 * ghostScale * collapseFactor;
                    float cOff1 = shDispersion * shMh * 0.04 * collapseFactor;
                    if (length(localPos - g1) < r1 + cOff1 + maxGhostFalloff) {
                        float3 g1Color;
                        if (chromaticGhosts) {
                            float g1G = gooShapeGhost(localPos, g1, r1, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g1R = gooShapeGhost(localPos, g1 + flareAxis * cOff1, r1, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g1B = gooShapeGhost(localPos, g1 - flareAxis * cOff1, r1, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g1Color = float3(g1R, g1G, g1B);
                        } else {
                            float g1v = gooShapeGhost(localPos, g1, r1, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g1Color = float3(g1v);
                        }
                        shFlare += g1Color * 0.35 * proxFade * frostDim * collapseFactor * flareTintMul;
                    }

                    // Ghost 2: mid-distance opposite
                    float2 g2 = shapes[sh].position + flareAxis * (-0.7 * shMh * ghostScale * collapseFactor);
                    float r2 = shMh * 0.10 * ghostScale * collapseFactor;
                    float cOff2 = shDispersion * shMh * 0.06 * collapseFactor;
                    if (length(localPos - g2) < r2 + cOff2 + maxGhostFalloff) {
                        float3 g2Color;
                        if (chromaticGhosts) {
                            float g2G = gooShapeGhost(localPos, g2, r2, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g2R = gooShapeGhost(localPos, g2 + flareAxis * cOff2, r2, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g2B = gooShapeGhost(localPos, g2 - flareAxis * cOff2, r2, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g2Color = float3(g2R, g2G, g2B);
                        } else {
                            float g2v = gooShapeGhost(localPos, g2, r2, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g2Color = float3(g2v);
                        }
                        shFlare += g2Color * 0.25 * proxFade * frostDim * collapseFactor * flareTintMul;
                    }

                    // Ghost 3: ring near center
                    float2 g3 = shapes[sh].position + flareAxis * (-0.2 * shMh * ghostScale * collapseFactor);
                    float r3 = shMh * 0.09 * ghostScale * collapseFactor;
                    if (length(localPos - g3) < r3 + maxGhostFalloff) {
                        float ringW = r3 * 0.2;
                        float ring = gooShapeRing(localPos, g3, r3, ringW, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                        shFlare += float3(0.5, 0.7, 1.0) * flareTintMul * ring * 0.2 * proxFade * frostDim * collapseFactor;
                    }

                    // Ghost 4: far opposite wash
                    float2 g4 = shapes[sh].position + flareAxis * (-1.0 * shMh * ghostScale * collapseFactor);
                    float r4 = shMh * 0.18 * ghostScale * collapseFactor;
                    if (length(localPos - g4) < r4 + maxGhostFalloff) {
                        float g4v = gooShapeGhost(localPos, g4, r4, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                        shFlare += float3(0.7, 0.85, 1.0) * flareTintMul * g4v * 0.18 * proxFade * frostDim * collapseFactor;
                    }

                    // Ghost 5: tiny chromatic point near opposite edge
                    float2 g5 = shapes[sh].position + flareAxis * (-1.2 * shMh * ghostScale * collapseFactor);
                    float r5 = shMh * 0.05 * ghostScale * collapseFactor;
                    float cOff5 = shDispersion * shMh * 0.08 * collapseFactor;
                    if (length(localPos - g5) < r5 + cOff5 + maxGhostFalloff) {
                        float3 g5Color;
                        if (chromaticGhosts) {
                            float g5R = gooShapeGhost(localPos, g5 + flareAxis * cOff5, r5, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g5G = gooShapeGhost(localPos, g5, r5, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            float g5B = gooShapeGhost(localPos, g5 - flareAxis * cOff5, r5, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g5Color = float3(g5R, g5G, g5B);
                        } else {
                            float g5v = gooShapeGhost(localPos, g5, r5, shapes[sh], sdf0, sdf1, sdf2, sdf3, sdf4, sdf5, sdf6, sdf7);
                            g5Color = float3(g5v);
                        }
                        shFlare += g5Color * 0.3 * proxFade * frostDim * collapseFactor * flareTintMul;
                    }
                }

                flare += shFlare * shWeight;
                flareWeightTotal += shWeight;
            }

            flare /= max(flareWeightTotal, 1e-6);
        }

        // ── Rim Light (inline, unified surface) ──────────────────────────
        // Computed directly from the merged SDF surface normal so lighting is
        // coherent across all shape types and multi-shape meld zones.
        {
            float2 groupCenter = in.worldPos - localPos;
            float2 lightDirWorld;
            if (goo.cursorActive > 1.5) {
                // Directional mode: cursorWorldPos IS the direction (not a position).
                // No groupCenter subtraction — just normalize the direction vector.
                float tcLen = length(goo.cursorWorldPos);
                lightDirWorld = tcLen > 0.001 ? goo.cursorWorldPos / tcLen : float2(0.0, -1.0);
            } else if (goo.cursorActive > 0.5) {
                // Point cursor mode: compute direction from group center to cursor position.
                float2 toCursor = goo.cursorWorldPos - groupCenter;
                float tcLen = length(toCursor);
                lightDirWorld = tcLen > 0.001 ? toCursor / tcLen : float2(0.0, -1.0);
            } else {
                lightDirWorld = float2(sin(mat.lightAngle), -cos(mat.lightAngle));
            }

            float2 litEdgeDir = float2(-lightDirWorld.y, lightDirWorld.x);
            float2 absEdge = abs(litEdgeDir);

            // Softmax-blend geometric rim quantities across all shapes
            float blendTemp = max(goo.smoothK * 0.5, 1.0);
            float litEdgeLen = 0.0, shapeDist = 0.0, blendedRadius = 0.0, blendTotalW = 0.0;
            float2 blendedCenter = float2(0.0);
            for (uint bi = 0; bi < goo.shapeCount; bi++) {
                float bsd = cachedSDF[bi];
                float bw = exp(-bsd / blendTemp);
                float2 bHalf = shapes[bi].halfSize;
                float2 bRel = localPos - shapes[bi].position;
                float bEdgeLen = (shapes[bi].shapeType == 1)
                    ? max(length(bHalf * absEdge), 1.0)
                    : max(dot(bHalf, absEdge), 1.0);
                float bShapeDist;
                if (shapes[bi].shapeType == 1 && bsd <= 0.0) {
                    float2 be = max(bHalf, float2(1e-4));
                    float2 bpe = bRel / be;
                    float bImp = dot(bpe, bpe) - 1.0;
                    bShapeDist = max(-bImp, 0.0) * min(bHalf.x, bHalf.y) * 0.5;
                } else {
                    bShapeDist = max(-bsd, 0.0);
                }
                litEdgeLen    += bEdgeLen * bw;
                shapeDist     += bShapeDist * bw;
                blendedCenter += shapes[bi].position * bw;
                blendedRadius += length(bHalf) * bw;
                blendTotalW   += bw;
            }
            float invBlendW = 1.0 / max(blendTotalW, 1e-6);
            litEdgeLen    *= invBlendW;
            shapeDist     *= invBlendW;
            blendedCenter *= invBlendW;
            blendedRadius *= invBlendW;

            float rimEdgeFade = smoothstep(0.0, 1.0, max(-d, 0.0));

            float rimWidth  = max(litEdgeLen * 0.10, 4.0);
            float gleamWidth = max(litEdgeLen * 0.40, 10.0);

            float normalDotLight = dot(surfaceNormal, lightDirWorld);
            float fresnelGraze = 1.0 - abs(normalDotLight);
            float fg2 = fresnelGraze * fresnelGraze;
            float rimEdgeProxNorm = clamp(interiorDistance / max(maxInteriorDist, 1.0), 0.0, 1.0);
            float fresnelEdgeMask = 1.0 - smoothstep(0.0, 0.35, rimEdgeProxNorm);
            float fresnelBoost = mix(0.5, 1.2, fg2 * fresnelEdgeMask);
            float variantLightScale = (goo.glassVariant == 1) ? 0.70 : 1.0;

            float si = mat.lightIntensity, li = mat.lightIntensity, lb = mat.lightBanding;
            float bandScale = mix(0.25, 1.0, lb);
            float fRimW  = rimWidth  * mix(0.3, 1.5, fg2);
            float fGleamW = gleamWidth * mix(0.4, 1.3, fg2);
            float sRimW, sGleamW;
            if (goo.cursorActive > 0.5) {
                sRimW  = max(mix(0.35, rimWidth  * bandScale, si), 1.5);
                sGleamW = max(mix(0.75, gleamWidth * bandScale, si), 3.0);
            } else {
                sRimW  = max(mix(0.35, fRimW  * bandScale, si), 1.5);
                sGleamW = max(mix(0.75, fGleamW * bandScale, si), 3.0);
            }
            // Use the merged SDF interior distance for rim/gleam falloff so the
            // bridge zone is treated as deep interior (no spurious rim). The per-shape
            // shapeDist is near-zero in the bridge because individual SDFs are positive
            // there, which would incorrectly light up the bridge seam.
            float unifiedDepth = max(-d, 0.0);
            float sRimFalloff   = exp(-unifiedDepth * unifiedDepth / (sRimW   * sRimW));
            float sGleamFalloff = exp(-unifiedDepth * unifiedDepth / (sGleamW * sGleamW));
            float ambientBase = mix(0.15, 0.30, si);
            float liBright    = mix(0.0,  1.0,  li);
            float3 lightContrib = float3(0.0);

            // ── Ambient rim — always-on ~1px edge line, full circumference ──
            // Uses d directly (SDF: negative inside, 0 at surface).
            // Intensity scales with lightIntensity (si) and lightBanding (lb).
            {
                float depth  = max(-d, 0.0);  // 0 at surface, positive inside
                float onePx  = max(length(dfdx(localPos)), 0.5);
                float ambW   = onePx * mix(1.2, 2.0, lb);
                float ambMask = exp(-depth * depth / max(ambW * ambW, 0.25));
                float ambStr  = mix(0.08, 0.30, lb) * si * variantLightScale;
                lightContrib += float3(ambStr * ambMask);
            }

            if (goo.cursorActive > 0.5) {
                float ghostDelta = max(length(dfdx(localPos)), 0.5) * 3.0;

                // Project pixel onto boundary → pixelClosest
                // On bridges the gradient can jump to a distant blob, so blend
                // toward localPos when the pixel is near the surface (d ≈ 0).
                // Reuse surfaceNormal (normalized unified gradient) from above
                // instead of recomputing gooUnifiedGradient at a wider delta.
                float2 projected    = localPos - d * surfaceNormal;
                float bridgeBlend   = smoothstep(0.0, ghostDelta * 2.0, abs(d));
                float2 pixelClosest = mix(localPos, projected, bridgeBlend);

                float2 cursorLocal = goo.cursorWorldPos - groupCenter;
                float depth = max(-d, 0.0);
                float bloomW  = mix(2.0, 8.0, lb) * max(length(dfdx(localPos)), 0.5);
                float bloomMask = exp(-depth * depth / max(bloomW * bloomW, 1.0));

                if (goo.cursorActive > 1.5) {
                    // ── Directional mode (tilt-driven) ──
                    // No point cursor exists — light is a direction, not a position.
                    // Use normalDotLight for directional rim (same as standalone glass)
                    // instead of distance-based proximity which can't work for a
                    // direction-only input across spread-out shapes.
                    float hotspot  = pow(max(normalDotLight, 0.0), mix(8.0, 2.0, li));
                    float gleamSpot = pow(max(normalDotLight, 0.0), mix(5.0, 1.0, li));

                    // ── Near/far modulation from Y-axis tilt ──
                    // tiltY ranges −0.5 (tilted away) to +0.5 (tilted toward you).
                    // More tilt → near-side brightens, far-side dims.
                    // Like tilting a real glass under a lamp.
                    float tiltAmount = clamp(abs(goo.tiltY) * 2.0, 0.0, 1.0);
                    float nearBoost = mix(1.0, 1.5, tiltAmount);
                    float farDim    = mix(1.0, 0.3, tiltAmount);

                    float baseline   = ambientBase * sRimFalloff * rimEdgeFade * variantLightScale * 0.6;
                    float directional = hotspot  * sRimFalloff * rimEdgeFade * variantLightScale * 0.85 * fresnelBoost * liBright * nearBoost;
                    float gleam       = gleamSpot * sGleamFalloff * rimEdgeFade * variantLightScale * (0.25 * si) * fresnelBoost * liBright * nearBoost;
                    lightContrib += (baseline + directional + gleam) * float3(1.0, 1.0, 1.05);

                    // Faint chromatic far-side ghost driven by anti-normal direction.
                    float antiNdl = max(-normalDotLight, 0.0);
                    float farSpot = pow(antiNdl, mix(6.0, 2.0, li));
                    float farStr = mix(0.3, 0.8, lb) * si * liBright * variantLightScale * farDim;
                    // Chromatic split: shift normal tangentially for R and B channels.
                    float2 surfTan = safeNormalize(float2(-surfaceNormal.y, surfaceNormal.x));
                    float chromaSpread = mat.dispersion * 0.3;
                    float antiR = max(-dot(surfaceNormal + surfTan * chromaSpread, lightDirWorld), 0.0);
                    float antiB = max(-dot(surfaceNormal - surfTan * chromaSpread, lightDirWorld), 0.0);
                    float farChromaR = pow(antiR, mix(6.0, 2.0, li));
                    float farChromaB = pow(antiB, mix(6.0, 2.0, li));
                    float3 farContrib = float3(farChromaR, farSpot, farChromaB) * farStr * bloomMask;
                    lightContrib += farContrib;

                } else {
                    // ── Point cursor mode (mouse / Apple Pencil hover) ──
                    // Proximity bloom: cursor pushes brightness into the rim.
                    // Measured as distance from cursor to this pixel's nearest boundary point.
                    float distCursorToRim = length(cursorLocal - pixelClosest);
                    float nearR   = max(blendedRadius * mix(0.3, 0.7, lb), 40.0);
                    float nearProx = exp(-distCursorToRim * distCursorToRim / (nearR * nearR));
                    float bloomStr = nearProx * mix(0.4, 1.2, lb) * si * liBright * variantLightScale;
                    lightContrib += float3(bloomStr * bloomMask);

                    // Far ghost (lens flare): reflect cursor through blendedCenter.
                    float2 farCursor = 2.0 * blendedCenter - cursorLocal;
                    float distFarToRim = length(farCursor - pixelClosest);
                    float farR    = max(blendedRadius * mix(0.2, 0.5, lb), 30.0);
                    float farProx = exp(-distFarToRim * distFarToRim / (farR * farR));
                    float farMask = exp(-depth * depth / max(bloomW * bloomW, 1.0));

                    // Chromatic aberration on far ghost
                    float2 farTan = float2(-surfaceNormal.y, surfaceNormal.x);
                    float chromaSpread = mat.dispersion * farR * 0.35;
                    float2 pixelRc = pixelClosest - farTan * chromaSpread;
                    float2 pixelBc = pixelClosest + farTan * chromaSpread;
                    float distFarRc = length(farCursor - pixelRc);
                    float distFarBc = length(farCursor - pixelBc);
                    float farChromaR = exp(-distFarRc * distFarRc / (farR * farR));
                    float farChromaB = exp(-distFarBc * distFarBc / (farR * farR));

                    float farStr = mix(0.3, 0.8, lb) * si * liBright * variantLightScale;
                    float3 farContrib = float3(farChromaR, farProx, farChromaB) * farStr * farMask;
                    lightContrib += farContrib;
                }

            } else {
                float pixelAngle = atan2(localPos.x, -localPos.y);
                float angleDelta = pixelAngle - mat.lightAngle;
                angleDelta = angleDelta - 6.2831853 * floor((angleDelta + 3.1415927) / 6.2831853);
                float angularCos = cos(angleDelta);
                float hotspot  = pow(max(angularCos, 0.0), mix(8.0, 2.0, li));
                float gleamSpot = pow(max(angularCos, 0.0), mix(5.0, 1.0, li));
                float baseline   = ambientBase * sRimFalloff * rimEdgeFade * variantLightScale * 0.6;
                float directional = hotspot  * sRimFalloff * rimEdgeFade * variantLightScale * 0.85 * fresnelBoost * liBright;
                float gleam       = gleamSpot * sGleamFalloff * rimEdgeFade * variantLightScale * (0.25 * si) * fresnelBoost * liBright;
                lightContrib += (baseline + directional + gleam) * float3(1.0, 1.0, 1.05);
                float interiorMask = smoothstep(0.05, 0.4, rimEdgeProxNorm);
                lightContrib += interiorMask * max(angularCos, 0.0) * si * 0.12 * liBright * float3(1.0, 1.0, 1.03);
            }

            // ── Brilliance source rim highlights ──────────────────────────
            // When Brilliance lights are present, add directional rim glow
            // facing each light origin. surfaceNormal is world-space for goo.
            if (goo.brillianceCount > 0) {
                float2 bSources[4] = {
                    goo.brillianceSource0, goo.brillianceSource1,
                    goo.brillianceSource2, goo.brillianceSource3
                };
                float3 bRimTints[4] = {
                    goo.brillianceTint0, goo.brillianceTint1,
                    goo.brillianceTint2, goo.brillianceTint3
                };
                // Use blendedCenter (softmax-weighted toward nearest shape) so
                // angles are relative to each shape's vicinity, not the group center.
                float2 blendedCenterWorld = groupCenter + blendedCenter;
                float2 bPixelRel = in.worldPos - blendedCenterWorld;
                float bPixelAngle = atan2(bPixelRel.x, -bPixelRel.y);
                for (uint bi = 0; bi < goo.brillianceCount && bi < 4; bi++) {
                    float2 srcRel = bSources[bi] - blendedCenterWorld;
                    float srcLen = length(srcRel);
                    if (srcLen < 0.001) continue;
                    float srcAngle = atan2(srcRel.x, -srcRel.y);
                    float srcAngleDelta = bPixelAngle - srcAngle;
                    srcAngleDelta = srcAngleDelta - 6.2831853 * floor((srcAngleDelta + 3.1415927) / 6.2831853);
                    float srcAngularCos = cos(srcAngleDelta);
                    float srcHotspot = pow(max(srcAngularCos, 0.0), mix(8.0, 2.0, li));
                    float srcGleamSpot = pow(max(srcAngularCos, 0.0), mix(5.0, 1.0, li));
                    float srcProxFade = 1.0 - smoothstep(blendedRadius * 1.0, blendedRadius * 6.0, srcLen);
                    if (srcProxFade < 0.001) continue;

                    // Per-light rim tint: explicit tint > resonance > canvas sampling
                    float3 rimTint;
                    if (mat.tintOpacity > 0.001) {
                        rimTint = mat.tintColor.rgb;
                    } else if (goo.resonanceEnabled != 0) {
                        rimTint = resonanceTint;
                    } else {
                        rimTint = bRimTints[bi];
                    }
                    float3 rimColor = mix(float3(1.0, 1.0, 1.05), rimTint, 0.35);

                    float bDir = srcHotspot * sRimFalloff * rimEdgeFade
                               * variantLightScale * 0.85 * fresnelBoost * liBright * srcProxFade;
                    float bGleam = srcGleamSpot * sGleamFalloff * rimEdgeFade
                                 * variantLightScale * (0.25 * si) * fresnelBoost * liBright * srcProxFade;
                    lightContrib += (bDir + bGleam) * rimColor;
                }
            }

            bgColor += clamp(lightContrib, 0.0, 1.0);
        }

        // ── Glimmer Glow ──
        if (goo.glowIntensity > 0.001) {
            float2 glowUV = in.uv;
            float3 glowColor = glowTex.sample(s, glowUV).rgb;
            float glowStr = goo.glowIntensity;

            if (goo.glowBlendMode == 0) {
                bgColor = bgColor + glowColor * glowStr - bgColor * glowColor * glowStr;
            } else if (goo.glowBlendMode == 1) {
                bgColor += glowColor * glowStr;
            } else {
                float3 a = bgColor;
                float3 b = glowColor * glowStr;
                bgColor = (1.0 - 2.0 * b) * a * a + 2.0 * b * a;
            }
        }

        bgColor = clamp(bgColor, 0.0, 1.0);

        // ── Appearance contrast layer ────────────────────────────────────
        // Applied per-pixel within the SDF membrane so it covers bridges.
        // Dark uses multiply blend: darkens bright backdrop areas without
        // adding color, so the glass surface stays natural.
        // Light uses screen blend: brightens dark areas toward white.
        if (goo.appearanceMode == 2) {
            // Dark: multiply by inverted luminance. Bright backdrops darken,
            // dark backdrops pass through unchanged.
            float3 apparent = mix(bgSampled, mat.tintColor.rgb, mat.tintOpacity);
            float bgLum = dot(apparent, float3(0.2126, 0.7152, 0.0722));
            bgColor *= mix(1.0, 1.0 - bgLum, 0.5);
        } else if (goo.appearanceMode == 1) {
            // Light: screen blend toward white. screen(a,b) = a + b - a*b
            // Lifts dark areas while barely touching already-bright ones.
            float3 wash = float3(0.35);
            bgColor = bgColor + wash - bgColor * wash;
        }

        float glassAlpha = shapeMask * in.opacity;
        // Screen-blend the brilliance flare onto the glass surface.
        // screen(a,b) = a + b - a*b  — caps at 1.0 naturally and preserves
        // underlying glass colour better than pure additive.
        float3 glassSurface = bgColor * glassAlpha;
        float3 flareContrib = flare * in.opacity;
        float3 result = glassSurface + flareContrib - glassSurface * flareContrib;
        float flareMax = max(flare.r, max(flare.g, flare.b));
        float alpha = min(glassAlpha + flareMax * in.opacity, 1.0);
        return float4(result, alpha);
    }

    // ── Luminance Mask ────────────────────────────────────────────────────
    // Post-composite pass that reads the rendered glass surface and outputs
    // a per-pixel luminance mask. Used by the SwiftUI modifier to composite
    // white/black foreground content so each pixel adapts independently.
    //
    // Output: all channels = BT.709 luminance.
    //   1.0 = bright backdrop → show black text
    //   0.0 = dark backdrop  → show white text (base layer)
    fragment float4 fragment_glass_luminance_mask(
        GlassVertexOut in [[stage_in]],
        texture2d<float> bgTex [[texture(0)]],
        constant float4 &uvRect [[buffer(0)]],
        constant float4 &tintParams [[buffer(1)]]  // .rgb = linear tint color, .a = tintOpacity
    ) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);
        // Remap quad UV [0,1] to the glass region within the full backdrop texture.
        // uvRect = (originX, originY, width, height) in backdrop UV space.
        float2 sampleUV = uvRect.xy + in.uv * uvRect.zw;
        float3 bg = bgTex.sample(s, sampleUV).rgb;

        // Mix tint contribution: the glass surface shifts apparent luminance
        // proportionally to tintOpacity. When tintOpacity=0, mix returns bg unchanged.
        float3 apparent = mix(bg, tintParams.rgb, tintParams.a);

        float lum = dot(apparent, float3(0.2126, 0.7152, 0.0722));
        return float4(lum, lum, lum, lum);
    }

    // ── Passthrough (Over-Composite) ────────────────────────────────────
    // Samples the source texture at screenUV and outputs as-is.
    // Used with premultiplied alpha blending to composite one group's
    // output onto the accumulation texture for multi-pass glass rendering.
    fragment float4 fragment_passthrough(
        GlassVertexOut in [[stage_in]],
        texture2d<float> srcTex [[texture(0)]]
    ) {
        constexpr sampler s(filter::linear, address::clamp_to_edge);
        return srcTex.sample(s, in.screenUV) * in.opacity;
    }

    // ── AF Probe (GPU-side luminance sampling) ───────────────────────────
    // Samples the luminance mask at 5 points (center + 4 cardinal) for each
    // element in one GPU dispatch, eliminating per-element CPU getBytes calls.
    //
    // Input:  per-element UV center + half-span for the 5-point cross.
    // Output: per-element averaged luminance (single float).

    struct AFProbeInput {
        float2 uv;       // center UV in mask texture space
        float2 uvSpan;   // half-size of 5-point cross in UV space
    };

    struct AFProbeOutput {
        float luminance;
    };

    kernel void kernel_af_probe(
        texture2d<float, access::sample> maskTex [[texture(0)]],
        constant AFProbeInput *inputs [[buffer(0)]],
        device AFProbeOutput *outputs [[buffer(1)]],
        uint tid [[thread_position_in_grid]]
    ) {
        constexpr sampler s(filter::linear, address::clamp_to_edge);
        AFProbeInput inp = inputs[tid];
        float2 c = inp.uv;
        float dx = inp.uvSpan.x;
        float dy = inp.uvSpan.y;

        // 5-point cross: center + 4 cardinal directions
        float lum  = maskTex.sample(s, c).r;
        lum += maskTex.sample(s, c + float2(-dx, 0)).r;
        lum += maskTex.sample(s, c + float2( dx, 0)).r;
        lum += maskTex.sample(s, c + float2(0, -dy)).r;
        lum += maskTex.sample(s, c + float2(0,  dy)).r;

        outputs[tid].luminance = lum * 0.2;
    }

    """
    // swiftformat:enable all
}

// swiftlint:enable file_length
