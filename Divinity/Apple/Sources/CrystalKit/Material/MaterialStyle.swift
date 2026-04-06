// MaterialStyle.swift
// CrystalKit

import CoreGraphics

/// Unified style protocol for all CrystalKit materials.
/// Provides a common interface for code that works with any material type.
public protocol MaterialStyle: Sendable, Equatable {
    /// Whether the material needs a backdrop texture for refraction effects.
    var needsBackdrop: Bool { get }

    /// The refraction strength (0-1). Used for shared interaction effects.
    var refraction: CGFloat { get set }

    /// The dispersion strength (0-1). Used for shared interaction effects.
    var dispersion: CGFloat { get set }
}
