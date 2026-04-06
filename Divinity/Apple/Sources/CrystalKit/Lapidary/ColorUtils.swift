// ColorUtils.swift
// CrystalKit

import simd
import SwiftUI

/// Converts a SwiftUI `Color` to a premultiplied RGBA SIMD4 for Metal shaders.
func premultipliedRGBA(_ color: Color?) -> SIMD4<Float> {
    guard let color else { return .zero }

    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0

    #if canImport(AppKit) && !targetEnvironment(macCatalyst)
    NSColor(color).usingColorSpace(.sRGB)?.getRed(&r, green: &g, blue: &b, alpha: &a)
    #elseif canImport(UIKit)
    UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
    #endif

    let alpha = Float(a)
    return SIMD4<Float>(Float(r) * alpha, Float(g) * alpha, Float(b) * alpha, alpha)
}

/// Resolves a SwiftUI `Color` to sRGB `SIMD3<Float>` (each channel 0-1).
func sRGBComponents(_ color: Color) -> SIMD3<Float> {
    var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0

    #if canImport(AppKit) && !targetEnvironment(macCatalyst)
    NSColor(color).usingColorSpace(.sRGB)?.getRed(&r, green: &g, blue: &b, alpha: &a)
    #elseif canImport(UIKit)
    UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
    #endif

    return SIMD3<Float>(Float(r), Float(g), Float(b))
}

extension Color {
    /// Creates a SwiftUI Color from an sRGB `SIMD3<Float>` (each channel 0-1).
    init(linearSIMD rgb: SIMD3<Float>) {
        self.init(red: Double(rgb.x), green: Double(rgb.y), blue: Double(rgb.z))
    }
}
