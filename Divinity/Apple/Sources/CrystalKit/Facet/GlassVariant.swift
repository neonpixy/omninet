// FacetVariant.swift
// CrystalKit

/// The glass material variant. Consumers can define custom variants:
/// ```swift
/// extension FacetVariant {
///     static let frosted = FacetVariant(rawValue: "frosted")
/// }
/// ```
public struct FacetVariant: RawRepresentable, Codable, Hashable, Sendable {
    public var rawValue: String
    public init(rawValue: String) { self.rawValue = rawValue }

    /// Standard glass with full blur and refraction.
    public static let regular = FacetVariant(rawValue: "regular")
    /// Transparent glass with subtle effect.
    public static let clear = FacetVariant(rawValue: "clear")

    /// All built-in variants. Custom variants are not included.
    public static let builtIn: [FacetVariant] = [.regular, .clear]

    public var displayName: String {
        switch rawValue {
        case "regular": "Regular"
        case "clear": "Clear"
        default: rawValue.capitalized
        }
    }
}
