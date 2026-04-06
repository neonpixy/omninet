// IrisStyle.swift
// CrystalKit
//
// Configuration for a CrystalKit thin-film interference material.
// Controls film stack, point-based dimple field, and appearance.

import SwiftUI

// MARK: - IrisDimple

/// A single dimple in the interference field.
///
/// Each dimple is a cosine bell dome at a user-defined position in unit
/// coordinates (0-1). Multiple dimples form a unified metaball field in the
/// shader — where domes overlap they merge smoothly.
///
/// Dimples with the same `linkGroup` UUID share radius/depth edits.
public struct IrisDimple: Sendable, Equatable, Codable, Identifiable {

    public var id: UUID

    /// Position in unit coordinates (0-1 on each axis within the shape).
    public var position: CGPoint

    /// Radius of the cosine bell dome as fraction of the shape's smaller dimension.
    /// Range: 0.01–1.0.
    public var radius: CGFloat

    /// Depth/height of the dome. Positive = bump, negative = concavity.
    /// Range: -1.0–1.0.
    public var depth: CGFloat

    /// Optional link group. Dimples sharing a linkGroup UUID synchronize
    /// radius and depth edits — change one and all in the group update.
    public var linkGroup: UUID?

    public init(
        id: UUID = UUID(),
        position: CGPoint = CGPoint(x: 0.5, y: 0.5),
        radius: CGFloat = 0.4,
        depth: CGFloat = 0.5,
        linkGroup: UUID? = nil
    ) {
        self.id = id
        self.position = position
        self.radius = radius
        self.depth = depth
        self.linkGroup = linkGroup
    }
}

/// Maximum number of dimples sent to the GPU per shape.
/// Safe for 60fps on integrated GPUs.
public let irisDimpleMaxCount: Int = 16

// MARK: - IrisStyle

/// Configuration for a thin-film interference material.
///
/// Use static presets for common looks:
/// ```swift
/// .iris(.nacre)
/// .iris(.oilSlick, shape: .circle)
/// .iris(IrisStyle(dimples: [IrisDimple(position: .init(x: 0.3, y: 0.5))]))
/// ```
public struct IrisStyle: Sendable, Equatable {

    // ── Film Stack ──

    /// Number of superimposed thin films (1–6).
    /// More layers = richer interference patterns, higher GPU cost.
    /// 3 is the sweet spot for most uses.
    public var layerCount: UInt32

    /// Base optical thickness of each film layer.
    /// Controls which part of the spectrum dominates.
    /// Range: 0.3–3.0.
    public var baseThickness: CGFloat

    /// How much thickness varies between stacked layers.
    /// 0 = all layers identical (flat color). 1 = maximum variation (rich banding).
    /// Range: 0.0–1.0.
    public var thicknessSpread: CGFloat

    /// Global multiplier on film thickness. Shifts the entire spectral
    /// response — like viewing the same film under different conditions.
    /// Range: 0.5–3.0.
    public var thicknessScale: CGFloat

    // ── Dimples ──

    /// User-positioned dimple points that form the interference field.
    /// Each dimple is a cosine bell dome. The shader loops all dimples
    /// to build a unified metaball height field.
    public var dimples: [IrisDimple]

    // ── Appearance ──

    /// Color saturation. 0 = grayscale interference. 1 = full spectral color.
    public var intensity: CGFloat

    /// Overall brightness multiplier.
    public var brightness: CGFloat

    /// Edge vignette — how the effect fades toward shape edges.
    public var edgeFade: CGFloat

    /// Backdrop refraction strength. The dimple field acts as a lens on the
    /// content behind the material.
    public var refraction: CGFloat

    /// Chromatic aberration on refracted backdrop.
    /// 0 = uniform refraction. 1 = strong wavelength-dependent offset.
    public var dispersion: CGFloat

    /// Whether the effect animates over time (spectral drift).
    public var animated: Bool

    /// Speed of spectral color drift over time.
    public var shiftSpeed: CGFloat

    /// Overall opacity (0 = transparent, 1 = fully opaque material).
    public var opacity: CGFloat

    // ── Gleam Tracking ──

    /// How much cursor/hover/tilt input shifts ALL dimple positions.
    /// 0 = no interaction. 1 = dimples follow input fully.
    /// Range: 0.0–1.0.
    public var gleamInfluence: CGFloat

    /// Maximum distance (in unit coords) dimples can shift
    /// from their base positions due to Gleam input.
    /// Range: 0.0–0.5.
    public var gleamRadius: CGFloat

    public init(
        layerCount: UInt32 = 3,
        baseThickness: CGFloat = 1.0,
        thicknessSpread: CGFloat = 0.6,
        thicknessScale: CGFloat = 1.0,
        dimples: [IrisDimple] = [IrisDimple()],
        intensity: CGFloat = 0.85,
        brightness: CGFloat = 0.9,
        edgeFade: CGFloat = 0.15,
        refraction: CGFloat = 0.3,
        dispersion: CGFloat = 0.2,
        animated: Bool = true,
        shiftSpeed: CGFloat = 0.3,
        opacity: CGFloat = 1.0,
        gleamInfluence: CGFloat = 0.5,
        gleamRadius: CGFloat = 0.2
    ) {
        self.layerCount = layerCount
        self.baseThickness = baseThickness
        self.thicknessSpread = thicknessSpread
        self.thicknessScale = thicknessScale
        self.dimples = dimples
        self.intensity = intensity
        self.brightness = brightness
        self.edgeFade = edgeFade
        self.refraction = refraction
        self.dispersion = dispersion
        self.animated = animated
        self.shiftSpeed = shiftSpeed
        self.opacity = opacity
        self.gleamInfluence = gleamInfluence
        self.gleamRadius = gleamRadius
    }
}

// MARK: - Presets

extension IrisStyle {

    /// Subtle nacre — like the inside of a shell. Gentle, pearlescent.
    public static let nacre = IrisStyle(
        layerCount: 3,
        baseThickness: 1.2,
        thicknessSpread: 0.4,
        dimples: [IrisDimple(radius: 0.3, depth: 0.3)],
        intensity: 0.7,
        brightness: 0.85,
        refraction: 0.15,
        shiftSpeed: 0.15
    )

    /// Oil slick — loose, liquid, organic. Wide color bands.
    public static let oilSlick = IrisStyle(
        layerCount: 2,
        baseThickness: 0.8,
        thicknessSpread: 0.7,
        dimples: [IrisDimple(radius: 0.6, depth: 0.6)],
        intensity: 0.9,
        brightness: 0.95,
        refraction: 0.4
    )

    /// Beetle shell — tight, chitinous, precise. Dense spectral banding.
    public static let beetle = IrisStyle(
        layerCount: 4,
        baseThickness: 1.0,
        thicknessSpread: 0.3,
        thicknessScale: 1.4,
        dimples: [IrisDimple(radius: 0.25, depth: 0.7)],
        intensity: 0.95,
        brightness: 0.8,
        refraction: 0.2,
        dispersion: 0.3
    )

    /// Soap bubble — soft, transparent, ethereal. Minimal layers.
    public static let bubble = IrisStyle(
        layerCount: 1,
        baseThickness: 0.6,
        thicknessSpread: 0.0,
        dimples: [IrisDimple(radius: 0.5, depth: 0.4)],
        intensity: 0.8,
        brightness: 1.0,
        refraction: 0.5,
        dispersion: 0.15,
        shiftSpeed: 0.5,
        opacity: 0.7
    )

    /// Vivid — maximum spectral impact. For when subtlety isn't the point.
    public static let vivid = IrisStyle(
        layerCount: 3,
        baseThickness: 1.0,
        thicknessSpread: 0.8,
        thicknessScale: 1.2,
        dimples: [IrisDimple(radius: 0.45, depth: 0.6)],
        intensity: 1.0,
        brightness: 1.0,
        refraction: 0.35,
        dispersion: 0.25,
        shiftSpeed: 0.4
    )
}

// MARK: - Codable

extension IrisStyle: Codable {

    private enum CodingKeys: String, CodingKey {
        case layerCount, baseThickness, thicknessSpread, thicknessScale
        case dimples
        // Legacy scalar dimple keys for migration
        case dimpleCenterX, dimpleCenterY, dimpleRadius, dimpleDepth, dimpleRotation
        // Legacy ripple keys (ignored on decode, never encoded)
        case rippleFrequency, rippleAmplitude, rippleDamping, rippleSpeed
        case intensity, brightness, edgeFade, refraction, dispersion
        case animated, shiftSpeed, opacity
        case gleamInfluence, gleamRadius
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(layerCount, forKey: .layerCount)
        try c.encode(baseThickness, forKey: .baseThickness)
        try c.encode(thicknessSpread, forKey: .thicknessSpread)
        try c.encode(thicknessScale, forKey: .thicknessScale)
        try c.encode(dimples, forKey: .dimples)
        try c.encode(intensity, forKey: .intensity)
        try c.encode(brightness, forKey: .brightness)
        try c.encode(edgeFade, forKey: .edgeFade)
        try c.encode(refraction, forKey: .refraction)
        try c.encode(dispersion, forKey: .dispersion)
        try c.encode(animated, forKey: .animated)
        try c.encode(shiftSpeed, forKey: .shiftSpeed)
        try c.encode(opacity, forKey: .opacity)
        try c.encode(gleamInfluence, forKey: .gleamInfluence)
        try c.encode(gleamRadius, forKey: .gleamRadius)
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        // Silent migration: V0.1 keys (palette, seed, spiralTightness, distortion,
        // noiseOctaves, highlight) and V0.3 ripple keys are silently ignored.
        layerCount = try c.decodeIfPresent(UInt32.self, forKey: .layerCount) ?? 3
        baseThickness = try c.decodeIfPresent(CGFloat.self, forKey: .baseThickness) ?? 1.0
        thicknessSpread = try c.decodeIfPresent(CGFloat.self, forKey: .thicknessSpread) ?? 0.6
        thicknessScale = try c.decodeIfPresent(CGFloat.self, forKey: .thicknessScale) ?? 1.0

        // Dimple migration: try array first, fall back to legacy scalar fields
        if let arr = try c.decodeIfPresent([IrisDimple].self, forKey: .dimples) {
            dimples = arr
        } else {
            // Legacy V0.3: synthesize single dimple from scalar fields
            let cx = try c.decodeIfPresent(CGFloat.self, forKey: .dimpleCenterX) ?? 0.5
            let cy = try c.decodeIfPresent(CGFloat.self, forKey: .dimpleCenterY) ?? 0.5
            let r = try c.decodeIfPresent(CGFloat.self, forKey: .dimpleRadius) ?? 0.4
            let d = try c.decodeIfPresent(CGFloat.self, forKey: .dimpleDepth) ?? 0.5
            dimples = [IrisDimple(position: CGPoint(x: cx, y: cy), radius: r, depth: d)]
        }

        intensity = try c.decodeIfPresent(CGFloat.self, forKey: .intensity) ?? 0.85
        brightness = try c.decodeIfPresent(CGFloat.self, forKey: .brightness) ?? 0.9
        edgeFade = try c.decodeIfPresent(CGFloat.self, forKey: .edgeFade) ?? 0.15
        refraction = try c.decodeIfPresent(CGFloat.self, forKey: .refraction) ?? 0.3
        dispersion = try c.decodeIfPresent(CGFloat.self, forKey: .dispersion) ?? 0.2
        animated = try c.decodeIfPresent(Bool.self, forKey: .animated) ?? true
        shiftSpeed = try c.decodeIfPresent(CGFloat.self, forKey: .shiftSpeed) ?? 0.3
        opacity = try c.decodeIfPresent(CGFloat.self, forKey: .opacity) ?? 1.0
        gleamInfluence = try c.decodeIfPresent(CGFloat.self, forKey: .gleamInfluence) ?? 0.5
        gleamRadius = try c.decodeIfPresent(CGFloat.self, forKey: .gleamRadius) ?? 0.2
    }
}

// MARK: - Style Delta

/// Additive delta for IrisStyle, used by IrisStylesheet cascade.
/// `nil` = no change; non-nil = additive, clamped to valid range after application.
/// Dimples are not affected by deltas (they are per-instance positioned).
public struct IrisStyleDelta: Sendable, Equatable, Codable {

    // Film Stack
    public var baseThicknessDelta: CGFloat?
    public var thicknessSpreadDelta: CGFloat?
    public var thicknessScaleDelta: CGFloat?

    // Appearance
    public var intensityDelta: CGFloat?
    public var brightnessDelta: CGFloat?
    public var edgeFadeDelta: CGFloat?
    public var refractionDelta: CGFloat?
    public var dispersionDelta: CGFloat?
    public var shiftSpeedDelta: CGFloat?
    public var opacityDelta: CGFloat?

    // Gleam
    public var gleamInfluenceDelta: CGFloat?
    public var gleamRadiusDelta: CGFloat?

    public static let identity = IrisStyleDelta()

    public init(
        baseThicknessDelta: CGFloat? = nil,
        thicknessSpreadDelta: CGFloat? = nil,
        thicknessScaleDelta: CGFloat? = nil,
        intensityDelta: CGFloat? = nil,
        brightnessDelta: CGFloat? = nil,
        edgeFadeDelta: CGFloat? = nil,
        refractionDelta: CGFloat? = nil,
        dispersionDelta: CGFloat? = nil,
        shiftSpeedDelta: CGFloat? = nil,
        opacityDelta: CGFloat? = nil,
        gleamInfluenceDelta: CGFloat? = nil,
        gleamRadiusDelta: CGFloat? = nil
    ) {
        self.baseThicknessDelta = baseThicknessDelta
        self.thicknessSpreadDelta = thicknessSpreadDelta
        self.thicknessScaleDelta = thicknessScaleDelta
        self.intensityDelta = intensityDelta
        self.brightnessDelta = brightnessDelta
        self.edgeFadeDelta = edgeFadeDelta
        self.refractionDelta = refractionDelta
        self.dispersionDelta = dispersionDelta
        self.shiftSpeedDelta = shiftSpeedDelta
        self.opacityDelta = opacityDelta
        self.gleamInfluenceDelta = gleamInfluenceDelta
        self.gleamRadiusDelta = gleamRadiusDelta
    }
}

extension IrisStyle {

    /// Returns a new style with the delta applied additively, clamped to valid ranges.
    public func applying(_ delta: IrisStyleDelta) -> IrisStyle {
        var r = self
        if let d = delta.baseThicknessDelta {
            r.baseThickness = max(0.3, min(3.0, baseThickness + d))
        }
        if let d = delta.thicknessSpreadDelta {
            r.thicknessSpread = max(0, min(1, thicknessSpread + d))
        }
        if let d = delta.thicknessScaleDelta {
            r.thicknessScale = max(0.5, min(3.0, thicknessScale + d))
        }
        if let d = delta.intensityDelta {
            r.intensity = max(0, min(1, intensity + d))
        }
        if let d = delta.brightnessDelta {
            r.brightness = max(0, min(1, brightness + d))
        }
        if let d = delta.edgeFadeDelta {
            r.edgeFade = max(0, min(1, edgeFade + d))
        }
        if let d = delta.refractionDelta {
            r.refraction = max(0, min(1, refraction + d))
        }
        if let d = delta.dispersionDelta {
            r.dispersion = max(0, min(1, dispersion + d))
        }
        if let d = delta.shiftSpeedDelta {
            r.shiftSpeed = max(0, min(3, shiftSpeed + d))
        }
        if let d = delta.opacityDelta {
            r.opacity = max(0, min(1, opacity + d))
        }
        if let d = delta.gleamInfluenceDelta {
            r.gleamInfluence = max(0, min(1, gleamInfluence + d))
        }
        if let d = delta.gleamRadiusDelta {
            r.gleamRadius = max(0, min(0.5, gleamRadius + d))
        }
        return r
    }
}
