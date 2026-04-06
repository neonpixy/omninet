// FacetStyle.swift
// CrystalKit

import simd
import SwiftUI

/// How the glass light direction is determined.
public enum LightSource: Codable, Sendable, Equatable {
    /// Static light at the fixed `lightRotation` and `lightIntensity` values (default).
    case fixed

    /// Light angle and intensity track cursor position (macOS/iPadOS) or gaze (iPhone).
    /// - `falloffRadius`: Distance in points where intensity fades to `baseIntensity`.
    ///   `nil` = angle-only tracking (no distance-based intensity falloff).
    /// - `baseIntensity`: Minimum intensity when cursor/gaze is far away (0-1).
    case cursor(falloffRadius: CGFloat? = 300, baseIntensity: CGFloat = 0.3)

    // MARK: - Codable

    private enum CodingKeys: String, CodingKey {
        case type, falloffRadius, baseIntensity
    }

    private enum SourceType: String, Codable {
        case fixed, cursor
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .fixed:
            try container.encode(SourceType.fixed, forKey: .type)
        case .cursor(let falloffRadius, let baseIntensity):
            try container.encode(SourceType.cursor, forKey: .type)
            try container.encodeIfPresent(falloffRadius, forKey: .falloffRadius)
            try container.encode(baseIntensity, forKey: .baseIntensity)
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(SourceType.self, forKey: .type)
        switch type {
        case .fixed:
            self = .fixed
        case .cursor:
            let falloffRadius = try container.decodeIfPresent(CGFloat.self, forKey: .falloffRadius)
            let baseIntensity = try container.decodeIfPresent(CGFloat.self, forKey: .baseIntensity) ?? 0.3
            self = .cursor(falloffRadius: falloffRadius, baseIntensity: baseIntensity)
        }
    }
}

/// Glass appearance mode — controls foreground color and contrast layer.
///
/// Built-in modes handle common scenarios. Consumers can define custom modes:
/// ```swift
/// extension FacetAppearance {
///     static let highContrast = FacetAppearance(rawValue: "highContrast")
/// }
/// ```
///
/// The renderer resolves known built-in values (`.light`, `.dark`, `.auto`, `.base`)
/// and falls back to `.base` behavior for unknown custom values.
public struct FacetAppearance: RawRepresentable, Codable, Hashable, Sendable {
    public var rawValue: String
    public init(rawValue: String) { self.rawValue = rawValue }

    /// No appearance enforcement — foreground colors are unmodified.
    public static let base = FacetAppearance(rawValue: "base")
    /// Light glass: dark foreground content.
    public static let light = FacetAppearance(rawValue: "light")
    /// Dark glass: white foreground + contrast shadow layer inside the glass.
    public static let dark = FacetAppearance(rawValue: "dark")
    /// Follows the system appearance (`ColorScheme.light` → `.light`, `.dark` → `.dark`).
    public static let auto = FacetAppearance(rawValue: "auto")

    /// All built-in appearance modes. Custom modes are not included.
    public static let builtIn: [FacetAppearance] = [.base, .light, .dark, .auto]
}

/// Configuration for a CrystalKit liquid glass effect.
///
/// Controls the visual properties of the glass material: blur intensity (frost),
/// background warping (refraction), chromatic aberration (dispersion), lens thickness
/// (depth), radial distortion (splay), and lighting (rotation + intensity).
///
/// Use the static presets for common configurations:
/// ```swift
/// .facet(.regular)
/// .facet(.frosted)
/// .facet(FacetStyle(frost: 0.7, refraction: 0.4))
/// ```
public struct FacetStyle: Sendable, Equatable {

    /// How the light direction is determined (`.fixed` or `.cursor(...)`).
    /// When `.cursor(...)`, the view tracks mouse/pointer/gaze position and
    /// dynamically overrides `lightRotation` and `lightIntensity` each frame.
    public var lightSource: LightSource = .fixed

    /// The glass material variant (regular or clear).
    public var variant: FacetVariant

    /// Optional tint color for the glass surface.
    public var tintColor: Color?

    /// Tint blending amount (0-1). Only applies when `tintColor` is set.
    public var tintOpacity: CGFloat

    /// Corner radius override. If `nil`, inferred from the clipping shape.
    public var cornerRadius: CGFloat?

    // MARK: - Shader Properties

    /// How much background content bends through the glass (0-1).
    /// Higher values create stronger lens-like warping at the edges.
    public var refraction: CGFloat

    /// How much the background is blurred/obscured (0-1).
    /// 0 = clear glass, 1 = fully frosted.
    public var frost: CGFloat

    /// Chromatic color separation at glass edges (0-1).
    /// Simulates prismatic refraction — rainbow fringing where the glass curves.
    public var dispersion: CGFloat

    /// Magnification depth of the glass lens (0-1).
    /// 0 = no magnification, 1 = strong zoom effect.
    public var depth: CGFloat

    /// Radial barrel distortion of the background (0-1).
    /// Simulates how thick glass splays light outward from center.
    public var splay: CGFloat

    /// Rotation angle of the light source (0-1 maps to 0°-360°).
    /// 0 = top, 0.25 = right, 0.5 = bottom, 0.75 = left.
    public var lightRotation: CGFloat

    /// Intensity of rim light and inner shadow (0-1).
    /// Controls how bright the light is and how far along the edge it extends.
    public var lightIntensity: CGFloat

    /// Gradient falloff of the rim light band (0-1).
    /// 0 = sharp, crisp cutoff (razor edge). 1 = soft, feathered fade (atmospheric).
    public var lightBanding: CGFloat

    /// How far refraction extends inward from shape edges (0-1).
    /// 0 = thin band at edges, 1 = reaches deep into center.
    public var edgeWidth: CGFloat

    /// Adaptive tinting based on background luminance.
    /// When enabled, dark backgrounds tint toward the lightest sampled color
    /// and light backgrounds tint toward the darkest, improving legibility.
    public var resonance: Bool

    /// Inner light/shadow driven by backdrop luminance.
    /// When enabled, bright backdrop regions add a warm inner glow and dark
    /// regions deepen the glass, creating a flowing light field across the surface.
    public var luminance: Bool

    /// World-space positions of light sources for the brilliance (lens flare) effect.
    /// Each glass node computes its own flare axis from its center toward each source.
    /// Empty array = brilliance disabled. Max 4 sources are sent to the GPU.
    public var brillianceSources: [SIMD2<Float>]

    /// Glass appearance mode. Controls foreground color and whether a contrast
    /// shadow layer is composited inside the glass. `.auto` follows the system
    /// `ColorScheme`, connecting CrystalKit to macOS/iOS Light/Dark mode.
    public var appearance: FacetAppearance

    /// Whether the renderer needs to generate a per-pixel luminance mask.
    public var needsLuminanceMask: Bool {
        appearance != .base
    }

    /// When true, the backdrop and luminance mask regenerate periodically
    /// (~every 66ms) instead of only on content change. Enable when the glass
    /// overlays video, live camera, or other animated content.
    public var dynamicBackdrop: Bool

    public init(
        variant: FacetVariant = .regular,
        tintColor: Color? = nil,
        tintOpacity: CGFloat = 0.0,
        cornerRadius: CGFloat? = nil,
        refraction: CGFloat = 0.35,
        frost: CGFloat = 0.5,
        dispersion: CGFloat = 0.15,
        depth: CGFloat = 0.0,
        splay: CGFloat = 0.0,
        lightRotation: CGFloat = 0.6,
        lightIntensity: CGFloat = 0.6,
        lightBanding: CGFloat = 0.5,
        edgeWidth: CGFloat = 0.05,
        resonance: Bool = false,
        luminance: Bool = false,
        brillianceSources: [SIMD2<Float>] = [],
        appearance: FacetAppearance = .base,
        dynamicBackdrop: Bool = false
    ) {
        self.variant = variant
        self.tintColor = tintColor
        self.tintOpacity = tintOpacity
        self.cornerRadius = cornerRadius
        self.refraction = refraction
        self.frost = frost
        self.dispersion = dispersion
        self.depth = depth
        self.splay = splay
        self.lightRotation = lightRotation
        self.lightIntensity = lightIntensity
        self.lightBanding = lightBanding
        self.edgeWidth = edgeWidth
        self.resonance = resonance
        self.luminance = luminance
        self.brillianceSources = brillianceSources
        self.appearance = appearance
        self.dynamicBackdrop = dynamicBackdrop
    }
}

// MARK: - Presets

extension FacetStyle {
    /// Standard glass with default parameters. The everyday glass effect.
    public static let regular = FacetStyle()

    /// Clear/transparent glass with reduced blur and refraction.
    public static let clear = FacetStyle(
        variant: .clear,
        refraction: 0.2,
        frost: 0.3
    )

    /// Subtle glass — barely-there frosting and gentle refraction.
    public static let subtle = FacetStyle(
        refraction: 0.15,
        frost: 0.3,
        dispersion: 0.08,
        lightIntensity: 0.4
    )

    /// Heavy frost — strong blur, visible refraction. Think bathroom glass.
    public static let frosted = FacetStyle(
        refraction: 0.4,
        frost: 0.85,
        dispersion: 0.2,
        lightIntensity: 0.7
    )
}

// MARK: - Codable

extension FacetStyle: Codable {

    private enum CodingKeys: String, CodingKey {
        case lightSource, variant, tintColor, tintOpacity, cornerRadius
        case refraction, frost, dispersion, depth, splay
        case lightRotation, lightIntensity, lightBanding, edgeWidth
        case resonance, luminance, brillianceSources
        case appearance, dynamicBackdrop
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(lightSource, forKey: .lightSource)
        try container.encode(variant, forKey: .variant)
        try container.encodeIfPresent(tintColor.map(CodableColor.init), forKey: .tintColor)
        try container.encode(tintOpacity, forKey: .tintOpacity)
        try container.encodeIfPresent(cornerRadius, forKey: .cornerRadius)
        try container.encode(refraction, forKey: .refraction)
        try container.encode(frost, forKey: .frost)
        try container.encode(dispersion, forKey: .dispersion)
        try container.encode(depth, forKey: .depth)
        try container.encode(splay, forKey: .splay)
        try container.encode(lightRotation, forKey: .lightRotation)
        try container.encode(lightIntensity, forKey: .lightIntensity)
        try container.encode(lightBanding, forKey: .lightBanding)
        try container.encode(edgeWidth, forKey: .edgeWidth)
        try container.encode(resonance, forKey: .resonance)
        try container.encode(luminance, forKey: .luminance)
        try container.encode(brillianceSources.map { [$0.x, $0.y] }, forKey: .brillianceSources)
        try container.encode(appearance, forKey: .appearance)
        try container.encode(dynamicBackdrop, forKey: .dynamicBackdrop)
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        lightSource = try c.decodeIfPresent(LightSource.self, forKey: .lightSource) ?? .fixed
        variant = try c.decodeIfPresent(FacetVariant.self, forKey: .variant) ?? .regular
        if let codable = try c.decodeIfPresent(CodableColor.self, forKey: .tintColor) {
            tintColor = codable.color
        } else {
            tintColor = nil
        }
        tintOpacity = try c.decodeIfPresent(CGFloat.self, forKey: .tintOpacity) ?? 0.0
        cornerRadius = try c.decodeIfPresent(CGFloat.self, forKey: .cornerRadius)
        refraction = try c.decodeIfPresent(CGFloat.self, forKey: .refraction) ?? 0.35
        frost = try c.decodeIfPresent(CGFloat.self, forKey: .frost) ?? 0.5
        dispersion = try c.decodeIfPresent(CGFloat.self, forKey: .dispersion) ?? 0.15
        depth = try c.decodeIfPresent(CGFloat.self, forKey: .depth) ?? 0.0
        splay = try c.decodeIfPresent(CGFloat.self, forKey: .splay) ?? 0.0
        lightRotation = try c.decodeIfPresent(CGFloat.self, forKey: .lightRotation) ?? 0.6
        lightIntensity = try c.decodeIfPresent(CGFloat.self, forKey: .lightIntensity) ?? 0.6
        lightBanding = try c.decodeIfPresent(CGFloat.self, forKey: .lightBanding) ?? 0.5
        edgeWidth = try c.decodeIfPresent(CGFloat.self, forKey: .edgeWidth) ?? 0.05
        resonance = try c.decodeIfPresent(Bool.self, forKey: .resonance) ?? false
        luminance = try c.decodeIfPresent(Bool.self, forKey: .luminance) ?? false
        let rawSources = try c.decodeIfPresent([[Float]].self, forKey: .brillianceSources) ?? []
        brillianceSources = rawSources.compactMap { arr in
            guard arr.count >= 2 else { return nil }
            return SIMD2<Float>(arr[0], arr[1])
        }
        appearance = try c.decodeIfPresent(FacetAppearance.self, forKey: .appearance) ?? .base
        dynamicBackdrop = try c.decodeIfPresent(Bool.self, forKey: .dynamicBackdrop) ?? false
    }
}

// MARK: - Style Delta (Stylesheet Cascade)

/// Additive property delta for stylesheet cascading. Each non-nil field is added
/// to the corresponding `FacetStyle` property, clamped to 0-1.
///
/// Unlike `FacetModification` (which drives interaction physics with 3 fields),
/// this covers all numeric glass properties for full stylesheet cascade control.
public struct FacetStyleDelta: Codable, Sendable, Equatable {
    public var frostDelta: CGFloat?
    public var refractionDelta: CGFloat?
    public var dispersionDelta: CGFloat?
    public var depthDelta: CGFloat?
    public var splayDelta: CGFloat?
    public var lightRotationDelta: CGFloat?
    public var lightIntensityDelta: CGFloat?
    public var lightBandingDelta: CGFloat?
    public var edgeWidthDelta: CGFloat?
    public var tintOpacityDelta: CGFloat?

    public init(
        frostDelta: CGFloat? = nil,
        refractionDelta: CGFloat? = nil,
        dispersionDelta: CGFloat? = nil,
        depthDelta: CGFloat? = nil,
        splayDelta: CGFloat? = nil,
        lightRotationDelta: CGFloat? = nil,
        lightIntensityDelta: CGFloat? = nil,
        lightBandingDelta: CGFloat? = nil,
        edgeWidthDelta: CGFloat? = nil,
        tintOpacityDelta: CGFloat? = nil
    ) {
        self.frostDelta = frostDelta
        self.refractionDelta = refractionDelta
        self.dispersionDelta = dispersionDelta
        self.depthDelta = depthDelta
        self.splayDelta = splayDelta
        self.lightRotationDelta = lightRotationDelta
        self.lightIntensityDelta = lightIntensityDelta
        self.lightBandingDelta = lightBandingDelta
        self.edgeWidthDelta = edgeWidthDelta
        self.tintOpacityDelta = tintOpacityDelta
    }

    /// No-op delta — applying this changes nothing.
    public static let identity = FacetStyleDelta()
}

// MARK: - Applying Deltas

extension FacetStyle {
    /// Returns a copy with the stylesheet delta applied additively, clamped to 0-1.
    public func applying(_ delta: FacetStyleDelta) -> FacetStyle {
        var s = self
        if let d = delta.frostDelta { s.frost = min(max(s.frost + d, 0), 1) }
        if let d = delta.refractionDelta { s.refraction = min(max(s.refraction + d, 0), 1) }
        if let d = delta.dispersionDelta { s.dispersion = min(max(s.dispersion + d, 0), 1) }
        if let d = delta.depthDelta { s.depth = min(max(s.depth + d, 0), 1) }
        if let d = delta.splayDelta { s.splay = min(max(s.splay + d, 0), 1) }
        if let d = delta.lightRotationDelta { s.lightRotation = min(max(s.lightRotation + d, 0), 1) }
        if let d = delta.lightIntensityDelta { s.lightIntensity = min(max(s.lightIntensity + d, 0), 1) }
        if let d = delta.lightBandingDelta { s.lightBanding = min(max(s.lightBanding + d, 0), 1) }
        if let d = delta.edgeWidthDelta { s.edgeWidth = min(max(s.edgeWidth + d, -1), 1) }
        if let d = delta.tintOpacityDelta { s.tintOpacity = min(max(s.tintOpacity + d, 0), 1) }
        return s
    }
}

// MARK: - Interaction Style Modification

extension FacetStyle {
    /// Returns a copy of this style with additive modifications applied.
    /// Each non-nil delta is added to the corresponding property, clamped to 0-1.
    func applying(_ mod: FacetModification) -> FacetStyle {
        var s = self
        if let d = mod.frostDelta { s.frost = min(max(s.frost + d, 0), 1) }
        if let d = mod.refractionDelta { s.refraction = min(max(s.refraction + d, 0), 1) }
        if let d = mod.dispersionDelta { s.dispersion = min(max(s.dispersion + d, 0), 1) }
        return s
    }
}

