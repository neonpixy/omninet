// FacetStylesheet.swift
// CrystalKit

import SwiftUI

/// A cascade engine for consistent glass theming across an app.
///
/// `FacetStylesheet` provides a CSS-like cascade: set a base style once,
/// and every semantic role (panel, sidebar, controlBar, tile, overlay)
/// automatically derives an optically-differentiated variant.
///
/// ```swift
/// // In your root view:
/// let sheet = FacetStylesheet(base: .regular)
///
/// ContentView()
///     .facetStylesheet(sheet)
///
/// // In any child view:
/// .facet(.panel)
/// .facet(.controlBar, in: RoundedRectangle(cornerRadius: 12))
/// ```
///
/// Override individual roles when needed:
/// ```swift
/// sheet.overrides[.sidebar] = myCustomSidebarStyle
/// ```
///
/// Subclass to customize the cascade logic:
/// ```swift
/// class MyStylesheet: FacetStylesheet {
///     override func delta(for role: FacetRole) -> FacetStyleDelta {
///         // Custom optical deltas
///     }
/// }
/// ```
@Observable @MainActor
open class FacetStylesheet {

    /// Base style — the "body { }" rule. All roles cascade from this.
    public var base: FacetStyle

    /// Per-role explicit overrides. Set a role to bypass cascading entirely.
    public var overrides: [FacetRole: FacetStyle] = [:]

    public init(base: FacetStyle = .regular) {
        self.base = base
    }

    /// Resolved style for a role: explicit override if set, otherwise base + delta.
    public func style(for role: FacetRole) -> FacetStyle {
        if let explicit = overrides[role] {
            return explicit
        }
        return base.applying(delta(for: role))
    }

    /// Per-role deltas for cascade logic. Mutate to register deltas for custom roles.
    /// Built-in roles are pre-populated with optically-differentiated defaults:
    /// - **panel**: Identity (IS the base)
    /// - **controlBar**: More frost, less refraction — crisp text over dense UI
    /// - **sidebar**: Moderate frost, quieter light — anchored, not flashy
    /// - **tile**: Less frost, more sparkle — gem-like at small scale
    /// - **overlay**: Heavy frost, subtle depth — floats over other glass
    public var deltas: [FacetRole: FacetStyleDelta] = defaultDeltas

    /// Optical delta per role. Override in subclasses to customize cascade logic.
    /// Unknown/custom roles with no registered delta return `.identity` (base style).
    open func delta(for role: FacetRole) -> FacetStyleDelta {
        deltas[role] ?? .identity
    }

    /// Built-in deltas for the five standard roles.
    public static let defaultDeltas: [FacetRole: FacetStyleDelta] = [
        .panel: .identity,
        .controlBar: FacetStyleDelta(
            frostDelta: 0.15,
            refractionDelta: -0.10,
            dispersionDelta: -0.05,
            lightIntensityDelta: -0.10
        ),
        .sidebar: FacetStyleDelta(
            frostDelta: 0.10,
            refractionDelta: -0.05,
            lightIntensityDelta: -0.05
        ),
        .tile: FacetStyleDelta(
            frostDelta: -0.15,
            refractionDelta: 0.05,
            dispersionDelta: 0.05,
            lightIntensityDelta: 0.05
        ),
        .overlay: FacetStyleDelta(
            frostDelta: 0.20,
            depthDelta: 0.05,
            lightIntensityDelta: 0.05
        ),
    ]

    /// Shared default stylesheet used when no `.facetStylesheet()` is in the environment.
    @MainActor
    static let `default` = FacetStylesheet()
}

// MARK: - View Extension

extension View {
    /// Sets a glass stylesheet for all CrystalKit glass surfaces in this subtree.
    ///
    /// ```swift
    /// ContentView()
    ///     .facetStylesheet(mySheet)
    /// ```
    public func facetStylesheet(_ stylesheet: FacetStylesheet) -> some View {
        environment(stylesheet)
    }
}
