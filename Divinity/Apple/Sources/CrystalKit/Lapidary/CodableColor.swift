// CodableColor.swift
// CrystalKit

import CoreGraphics
import SwiftUI

/// A cross-platform Codable wrapper for SwiftUI's `Color`.
///
/// Stores sRGB red, green, blue, and alpha components (each 0–1).
/// Round-trips through JSON/Plist and bridges to/from SwiftUI `Color`.
///
/// ```swift
/// let encoded = try JSONEncoder().encode(CodableColor(.blue))
/// let decoded = try JSONDecoder().decode(CodableColor.self, from: encoded)
/// let color: Color = decoded.color
/// ```
public struct CodableColor: Codable, Sendable, Equatable, Hashable {

    /// Red component (0–1, sRGB).
    public var red: CGFloat

    /// Green component (0–1, sRGB).
    public var green: CGFloat

    /// Blue component (0–1, sRGB).
    public var blue: CGFloat

    /// Alpha component (0–1).
    public var alpha: CGFloat

    public init(red: CGFloat, green: CGFloat, blue: CGFloat, alpha: CGFloat = 1.0) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    /// Creates a `CodableColor` from a SwiftUI `Color`.
    /// Resolves the color to sRGB components using platform-native APIs.
    public init(_ color: Color) {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0

        #if canImport(AppKit) && !targetEnvironment(macCatalyst)
        NSColor(color).usingColorSpace(.sRGB)?.getRed(&r, green: &g, blue: &b, alpha: &a)
        #elseif canImport(UIKit)
        UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
        #endif

        self.red = r
        self.green = g
        self.blue = b
        self.alpha = a
    }

    /// The SwiftUI `Color` equivalent.
    public var color: Color {
        Color(.sRGB, red: Double(red), green: Double(green), blue: Double(blue), opacity: Double(alpha))
    }
}
