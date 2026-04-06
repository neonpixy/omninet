// IrisShader.swift
// CrystalKit
//
// Metal shader source for the Iris thin-film interference material.
// Simulates multi-layer optical thin films with a point-based dimple
// field that builds a unified metaball height surface.

enum IrisShaderSource {
    static let source = """
    #include <metal_stdlib>
    using namespace metal;

    // =====================================================================
    // MARK: - Structs
    // =====================================================================

    struct IrisViewportUniforms {
        float4x4 viewProjection;
    };

    struct IrisQuadInstance {
        float2 position;
        float2 size;
        float rotation;
        float opacity;
        float scale;
        float _pad;
    };

    struct IrisVertexOut {
        float4 position [[position]];
        float2 uv;       // 0-1 within the shape quad
        float2 screenUV; // 0-1 within the viewport (for backdrop sampling)
        float opacity;   // per-instance opacity from the quad instance
    };

    struct IrisDimpleDescriptor {
        float2 position;  // unit coords (0-1)
        float  radius;    // fraction of shape's smaller dimension
        float  depth;     // -1 to 1
    };

    struct IrisUniforms {
        // Shape SDF
        float2 size;
        float2 _alignPad0;
        float4 cornerRadii;
        uint   shapeType;
        uint   sides;
        float  innerRadius;
        float  outerRadius;
        float  polygonBorderRadius;
        float  starInnerBorderRadius;
        float  cornerSmoothing;
        float  _shapePad;

        // Film Stack
        uint   layerCount;
        float  baseThickness;
        float  thicknessSpread;
        float  thicknessScale;

        // Dimple Count
        uint   dimpleCount;
        float  _dimplePad;

        // Appearance
        float  intensity;
        float  brightness;
        float  edgeFade;
        float  refraction;
        float  dispersion;
        float  opacity;

        // Animation
        float  time;
        float  shiftSpeed;

        // Gleam Tracking
        float2 gleamPosition;
        float  gleamInfluence;
        float  gleamRadius;

        // Backdrop UV Mapping
        float2 cropUVOffset;
        float2 cropUVScale;
    };

    // =====================================================================
    // MARK: - SDF Functions
    // =====================================================================

    float sdGlassRect(float2 p, float2 b, float4 r) {
        float2 rr;
        rr.x = (p.x > 0.0) ? ((p.y > 0.0) ? r.z : r.y) : ((p.y > 0.0) ? r.w : r.x);
        rr.y = rr.x;
        float2 q = abs(p) - b + rr;
        return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - rr.x;
    }

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

    float sdEllipseIris(float2 p, float2 ab) {
        float2 pa = abs(p);
        float2 r = ab;
        if (abs(r.x - r.y) < 0.001) return length(pa) - r.x;
        float2 q = pa;
        if (r.x < r.y) { q = q.yx; r = r.yx; }
        float a2 = r.x * r.x;
        float b2 = r.y * r.y;
        float t = 0.7853981633;
        for (int i = 0; i < 3; i++) {
            float ct = cos(t); float st = sin(t);
            float2 e = float2(a2 - b2, 0.0) * float2(ct*ct*ct, st*st*st) / r;
            float2 w = q - e;
            float2 g = float2(r.x * ct, r.y * st) - e;
            float gLen = length(g);
            if (gLen > 0.0001)
                t += atan2(cross(float3(w,0), float3(g,0)).z, dot(w,g)) / gLen;
        }
        float2 closest = float2(r.x * cos(t), r.y * sin(t));
        float dist = length(pa - closest);
        float inside = (pa.x*pa.x)/a2 + (pa.y*pa.y)/b2;
        return (inside < 1.0) ? -dist : dist;
    }

    float sdPolygonIris(float2 p, uint sides, float radius, float borderRadius) {
        float n = float(max(sides, 3u));
        float an = 3.14159265 / n;
        float sector = 2.0 * an;
        float wrapped = atan2(p.x, p.y) + an;
        wrapped -= sector * floor(wrapped / sector);
        float bn = wrapped - an;
        float d = length(p) * cos(bn) - radius * cos(an);
        if (borderRadius > 0.0) d -= borderRadius;
        return d;
    }

    float sdStarIris(float2 p, uint points, float innerR, float outerR,
                     float borderRadius, float innerBorderRadius) {
        float n = float(max(points, 3u));
        float an = 3.14159265 / n;
        float angle = atan2(p.x, p.y);
        float sector = fmod(angle + 3.14159265, 2.0 * an);
        float2 outerVtx = float2(sin(0.0), cos(0.0)) * outerR;
        float2 innerVtx = float2(sin(an), cos(an)) * innerR;
        float r = length(p);
        float2 localP = float2(sin(sector), cos(sector)) * r;
        float d;
        if (sector < an) {
            float2 edge = innerVtx - outerVtx;
            float2 toP = localP - outerVtx;
            float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
            d = length(localP - (outerVtx + edge * t));
        } else {
            float2 outerVtx2 = float2(sin(2.0*an), cos(2.0*an)) * outerR;
            float2 edge = outerVtx2 - innerVtx;
            float2 toP = localP - innerVtx;
            float t = clamp(dot(toP, edge) / dot(edge, edge), 0.0, 1.0);
            d = length(localP - (innerVtx + edge * t));
        }
        float2 edgeA, edgeB;
        if (sector < an) { edgeA = outerVtx; edgeB = innerVtx; }
        else { edgeA = innerVtx; edgeB = float2(sin(2.0*an), cos(2.0*an)) * outerR; }
        float2 edgeDir = edgeB - edgeA;
        float2 toP2 = localP - edgeA;
        float crossVal = edgeDir.x * toP2.y - edgeDir.y * toP2.x;
        d = (crossVal > 0.0) ? d : -d;
        if (borderRadius > 0.0 || innerBorderRadius > 0.0) {
            float tNorm = sector / an;
            float tipBlend = abs(tNorm - 1.0);
            float br = mix(innerBorderRadius, borderRadius, tipBlend);
            if (br > 0.0) { d += br; d = max(d, 0.0); d -= br; }
        }
        return d;
    }

    float sdIrisShape(float2 localPos, float2 halfSize, float4 radii, constant IrisUniforms &iris) {
        float minHalf = min(halfSize.x, halfSize.y);
        switch (iris.shapeType) {
            case 1: return sdEllipseIris(localPos, halfSize);
            case 2: {
                float2 scale = halfSize / max(minHalf, 1e-4);
                return sdPolygonIris(localPos / scale, iris.sides, minHalf, iris.polygonBorderRadius);
            }
            case 3: {
                float2 scale = halfSize / max(minHalf, 1e-4);
                float outerR = minHalf * iris.outerRadius;
                float innerR = outerR * iris.innerRadius;
                return sdStarIris(localPos / scale, iris.sides, innerR, outerR,
                                  iris.polygonBorderRadius, iris.starInnerBorderRadius);
            }
            default: return sdSuperellipseRect(localPos, halfSize, radii, iris.cornerSmoothing);
        }
    }

    // =====================================================================
    // MARK: - Vertex Shader
    // =====================================================================

    vertex IrisVertexOut vertex_iris_quad(
        uint vid [[vertex_id]],
        constant IrisViewportUniforms &viewport [[buffer(0)]],
        constant IrisQuadInstance &inst [[buffer(1)]]
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

        IrisVertexOut out;
        out.position = clipPos;
        out.uv = uv;
        out.screenUV = float2(clipPos.x * 0.5 + 0.5, -clipPos.y * 0.5 + 0.5);
        out.opacity = inst.opacity;
        return out;
    }

    // =====================================================================
    // MARK: - Thin-Film Interference
    // =====================================================================

    // Iris-specific spectral weights — maps 7 wavelength bands to RGB.
    // These are NOT shared with Glass (which uses exp(-8*t^2) Gaussians
    // for prismatic dispersion). These are tuned for thin-film interference
    // evaluation: each band represents a narrow slice of the visible spectrum.
    constexpr constant float3 irisSpectralW[7] = {
        float3(0.000, 0.135, 1.000),   // 380nm — deep violet
        float3(0.001, 0.246, 0.796),   // ~445nm — blue
        float3(0.005, 0.527, 0.401),   // ~510nm — cyan-green
        float3(0.018, 1.000, 0.135),   // ~550nm — green (perceptual peak)
        float3(0.082, 0.527, 0.030),   // ~590nm — yellow
        float3(0.401, 0.246, 0.004),   // ~640nm — orange
        float3(1.000, 0.135, 0.000),   // ~700nm — red
    };

    // Wavelength centers for each band (nanometers)
    constexpr constant float irisWavelengths[7] = {
        380.0, 445.0, 510.0, 550.0, 590.0, 640.0, 700.0
    };

    // 5-band spectral weights for chromatic aberration on backdrop refraction.
    constexpr constant float3 irisRefrSpectralW[5] = {
        float3(0.000, 0.150, 1.000),
        float3(0.050, 0.600, 0.500),
        float3(0.100, 1.000, 0.100),
        float3(0.500, 0.600, 0.050),
        float3(1.000, 0.150, 0.000),
    };

    // =====================================================================
    // MARK: - Fragment Shader
    // =====================================================================

    fragment float4 fragment_iris(
        IrisVertexOut in [[stage_in]],
        texture2d<float> maskTex [[texture(0)]],
        texture2d<float> backdropTex [[texture(1)]],
        constant IrisUniforms &iris [[buffer(0)]],
        constant IrisDimpleDescriptor *dimples [[buffer(1)]]
    ) {
        float2 uv = in.uv;
        float2 halfSize = iris.size * 0.5;
        float2 localPos = (uv - 0.5) * iris.size;

        // ── 1. SDF Shape Evaluation ──

        float sdf;
        if (iris.shapeType == 4) {
            constexpr sampler s(filter::linear, address::clamp_to_edge);
            float sdfValue = maskTex.sample(s, uv).r;
            sdf = sdfValue * min(iris.size.x, iris.size.y) * 0.5;
        } else {
            sdf = sdIrisShape(localPos, halfSize, iris.cornerRadii, iris);
        }

        if (sdf > 0.5) return float4(0.0);

        float edgeAlpha = 1.0 - smoothstep(-0.5, 0.5, sdf);
        float interiorDist = max(-sdf, 0.0);
        float maxInterior = min(halfSize.x, halfSize.y);
        float normalizedDepth = clamp(interiorDist / max(maxInterior, 1.0), 0.0, 1.0);
        float edgeMask = smoothstep(0.0, iris.edgeFade * 0.5 + 0.01, normalizedDepth);

        // ── 2. Unified Dimple Field ──
        // All dimples form one metaball-like height field. Each is a cosine
        // bell dome at a user-defined position. Where domes overlap they
        // merge smoothly — one continuous surface drives thickness,
        // refraction, and spectral interference.

        float unifiedField = 0.0;
        float2 unifiedGrad = float2(0.0);

        uint count = min(iris.dimpleCount, 16u);
        for (uint i = 0; i < count; i++) {
            float2 dimplePos = (dimples[i].position - 0.5) * iris.size;
            float2 toDimple = localPos - dimplePos;
            float dimpleSpread = max(maxInterior * dimples[i].radius, maxInterior * 0.05);

            float dist = length(toDimple);
            float t = clamp(dist / max(dimpleSpread, 1.0), 0.0, 1.0);

            // Cosine bell dome: smooth falloff to zero at radius edge
            float dome = (t < 1.0)
                ? (1.0 + cos(t * M_PI_F)) * 0.5
                : 0.0;
            unifiedField += dome * dimples[i].depth * 3.0;

            // Gradient: slope points outward from dome center
            float2 dir = -normalize(toDimple + 1e-6);
            float slope = (t < 1.0) ? sin(t * M_PI_F) * 0.5 : 0.0;
            unifiedGrad += dir * slope * abs(dimples[i].depth) * 0.15;
        }

        // --- Unified outputs ---
        float pathLengthDelta = unifiedField;
        float2 lensOffset = unifiedGrad;
        float refractionThickness = length(unifiedGrad) * iris.refraction * 2.0;

        // ── 3. Film Stack Interference ──

        float3 totalColor = float3(0.0);
        float3 totalWeight = float3(0.0);

        for (uint layer = 0; layer < min(iris.layerCount, 6u); layer++) {
            float layerOffset = float(layer) / max(float(iris.layerCount - 1), 1.0);
            float layerThickness = iris.baseThickness
                + (layerOffset - 0.5) * iris.thicknessSpread;

            float effectiveThickness = (layerThickness + pathLengthDelta + refractionThickness)
                * iris.thicknessScale;

            for (uint band = 0; band < 7; band++) {
                float lambda = irisWavelengths[band];

                // Thin-film interference: intensity = cos^2(2*pi*n*t / lambda)
                // n = 1.5 (typical film refractive index)
                float n = 1.5;
                float opticalPath = 2.0 * n * effectiveThickness * 100.0;
                float phase = 2.0 * M_PI_F * opticalPath / lambda;

                phase += iris.time * iris.shiftSpeed * float(layer + 1) * 0.3;

                float interference = cos(phase) * 0.5 + 0.5;

                totalColor += irisSpectralW[band] * interference;
                totalWeight += irisSpectralW[band];
            }
        }

        float3 irisColor = totalColor / max(totalWeight, float3(0.001));

        // ── 4. Spectral Saturation & Brightness ──
        // Physical thin-film produces dim output (most bands are partially
        // destructive). Gamma lift makes the colors vivid for a design tool.
        irisColor = pow(saturate(irisColor), float3(0.45));

        float3 gray = float3(dot(irisColor, float3(0.2126, 0.7152, 0.0722)));
        irisColor = mix(gray, irisColor, iris.intensity);
        irisColor *= iris.brightness;

        // ── 5. Backdrop Refraction Through Dimple Field ──
        // The refracted backdrop is the base layer. Interference colors are
        // additive spectral highlights on top — like looking through iridescent
        // glass. Opacity controls how strong the spectral overlay is.

        float2 backdropUV = iris.cropUVOffset + in.screenUV * iris.cropUVScale;
        constexpr sampler bgSampler(filter::linear, address::clamp_to_edge);

        if (iris.refraction > 0.001 && !is_null_texture(backdropTex)) {
            // Dimple lens refraction (already combined in lensOffset)
            float2 refractionOffset = lensOffset * iris.refraction;
            float3 refractedColor;

            if (iris.dispersion > 0.001) {
                // 5-band chromatic aberration
                refractedColor = float3(0.0);
                float3 refractedWeight = float3(0.0);

                for (uint band = 0; band < 5; band++) {
                    float bandT = float(band) / 4.0;
                    float bandScale = 1.0 + (bandT - 0.5) * iris.dispersion * 0.3;
                    float2 bandUV = backdropUV + refractionOffset * bandScale;
                    bandUV = clamp(bandUV, float2(0.001), float2(0.999));

                    float3 samp = backdropTex.sample(bgSampler, bandUV).rgb;
                    refractedColor += samp * irisRefrSpectralW[band];
                    refractedWeight += irisRefrSpectralW[band];
                }
                refractedColor /= max(refractedWeight, float3(0.001));
            } else {
                // Simple single-sample refraction (fast path)
                float2 refUV = clamp(backdropUV + refractionOffset, float2(0.001), float2(0.999));
                refractedColor = backdropTex.sample(bgSampler, refUV).rgb;
            }

            // Composite: refracted backdrop base + interference spectral highlights
            irisColor = refractedColor + irisColor * iris.opacity;
        }

        // ── 6. Fresnel Edge Glow ──

        float fresnelWidth = clamp(maxInterior * 0.08, 4.0, 20.0);
        float fresnelZone = 1.0 - smoothstep(0.0, fresnelWidth, interiorDist);
        float tiltVal = clamp(fresnelZone, 0.0, 0.999);
        float cosTheta = sqrt(1.0 - tiltVal * tiltVal);

        float F0 = 0.04;
        float oneMinusCos = 1.0 - cosTheta;
        float omc2 = oneMinusCos * oneMinusCos;
        float fresnelVal = F0 + (1.0 - F0) * (omc2 * omc2 * oneMinusCos);

        // Rim color from interference at shifted thickness
        // (simulates different optical path at glancing angle)
        irisColor += irisColor * fresnelVal * 0.4;

        // ── 7. Final Composite ──

        irisColor = min(irisColor, float3(1.0));
        float alpha = edgeAlpha * edgeMask * in.opacity;
        return float4(irisColor * alpha, alpha);  // premultiplied alpha
    }
    """
}
