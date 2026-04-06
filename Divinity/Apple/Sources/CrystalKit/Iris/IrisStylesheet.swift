// IrisStylesheet.swift
// CrystalKit
//
// CSS-like cascade system for Iris materials. Base style + per-role deltas.
// Reuses FacetRole for semantic role names (panel, sidebar, overlay, etc.)
// so a single role vocabulary works across all CrystalKit materials.

import SwiftUI
import Observation

/// Cascading stylesheet for Iris materials.
///
/// Resolution order (later wins):
/// 1. `base` style (foundation for all roles)
/// 2. `deltas[role]` (additive modification for the role)
/// 3. `overrides[role]` (complete replacement, skips delta)
///
/// ```swift
/// let sheet = IrisStylesheet(base: .opalescent)
/// sheet.deltas[.sidebar] = IrisStyleDelta(intensityDelta: 0.2)
/// sheet.overrides[.overlay] = .vivid
/// ```
@Observable @MainActor
open class IrisStylesheet {

    /// Foundation style applied when no role-specific override exists.
    public var base: IrisStyle

    /// Complete style replacements per role. Bypasses delta cascade.
    public var overrides: [FacetRole: IrisStyle] = [:]

    /// Additive deltas per role, applied on top of `base`.
    public var deltas: [FacetRole: IrisStyleDelta] = [:]

    public init(base: IrisStyle = IrisStyle()) {
        self.base = base
    }

    /// Resolves the effective style for a given role.
    public func style(for role: FacetRole) -> IrisStyle {
        if let explicit = overrides[role] { return explicit }
        return base.applying(delta(for: role))
    }

    /// Returns the delta for a role, defaulting to `.identity` for unknown roles.
    open func delta(for role: FacetRole) -> IrisStyleDelta {
        deltas[role] ?? .identity
    }
}

// MARK: - Environment Key

private struct IrisStylesheetKey: EnvironmentKey {
    static let defaultValue: IrisStylesheet? = nil
}

extension EnvironmentValues {
    /// The iris stylesheet for this subtree, if any.
    public var irisStylesheet: IrisStylesheet? {
        get { self[IrisStylesheetKey.self] }
        set { self[IrisStylesheetKey.self] = newValue }
    }
}

extension View {
    /// Sets the iris stylesheet for this view subtree.
    public func irisStylesheet(_ sheet: IrisStylesheet) -> some View {
        environment(\.irisStylesheet, sheet)
    }
}
