// FacetRole.swift
// CrystalKit

/// Semantic role for a glass surface. Each role carries optically-differentiated
/// defaults that cascade from the stylesheet's base style via additive deltas.
///
/// Built-in roles cover common UI patterns. Consumers can define custom roles
/// without modifying CrystalKit:
/// ```swift
/// extension FacetRole {
///     static let header = FacetRole(rawValue: "header")
///     static let card = FacetRole(rawValue: "card")
/// }
/// ```
///
/// Use with the role-based modifier:
/// ```swift
/// .facet(.panel)
/// .facet(.controlBar, in: RoundedRectangle(cornerRadius: 12))
/// .facet(.header)  // custom role — gets base style unless delta registered
/// ```
public struct FacetRole: RawRepresentable, Codable, Hashable, Sendable {
    public var rawValue: String
    public init(rawValue: String) { self.rawValue = rawValue }

    /// Large content surface — the reference point. No delta applied.
    public static let panel = FacetRole(rawValue: "panel")

    /// Dense interactive strip — more frost, less refraction for crisp text.
    public static let controlBar = FacetRole(rawValue: "controlBar")

    /// Persistent navigation surface — moderate frost, quieter light.
    public static let sidebar = FacetRole(rawValue: "sidebar")

    /// Small repeated element — less frost, more sparkle at small scale.
    public static let tile = FacetRole(rawValue: "tile")

    /// Floating popover or sheet — heavy frost, subtle depth for a lifted feel.
    public static let overlay = FacetRole(rawValue: "overlay")

    /// All built-in roles. Custom roles are not included.
    public static let builtIn: [FacetRole] = [.panel, .controlBar, .sidebar, .tile, .overlay]
}
