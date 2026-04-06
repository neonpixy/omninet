// ResonanceEffect.swift
// CrystalKit
//
// Protocol-based pairwise interaction system for glass panels.
// Effects are registered at the scope level and evaluated for each
// pair of children within range. Results apply forces and/or style mods.
//
// Usage:
//   ConfluenceScope(effects: [.springAttract(), .autoFrost()]) {
//       panel1.facet().confluence()
//       panel2.facet().confluence()
//   }

import SwiftUI

// MARK: - Snapshot & Context

/// A snapshot of one glass child's state for pairwise evaluation.
public struct ResonanceSnapshot: Sendable {
    public let id: String
    public let frame: CGRect
    public let style: FacetStyle
    public let crystallized: Bool
    /// Declaration order (0 = backmost).
    public let zIndex: Int
    /// True if the child's frame is changing due to user drag.
    public let isDragging: Bool

    public init(
        id: String,
        frame: CGRect,
        style: FacetStyle,
        crystallized: Bool,
        zIndex: Int,
        isDragging: Bool
    ) {
        self.id = id
        self.frame = frame
        self.style = style
        self.crystallized = crystallized
        self.zIndex = zIndex
        self.isDragging = isDragging
    }
}

/// Context for evaluating a pairwise interaction between two glass children.
public struct ResonanceContext: Sendable {
    /// The back panel (lower zIndex).
    public let childA: ResonanceSnapshot
    /// The front panel (higher zIndex).
    public let childB: ResonanceSnapshot
    /// Center-to-center distance in points.
    public let distance: CGFloat
    /// Normalized frame overlap area (0 = no overlap, 1 = fully overlapping).
    public let overlap: CGFloat
    /// Unit vector from A's center to B's center.
    public let direction: CGVector
    /// Relative velocity of A and B projected onto the center-to-center axis (pt/s).
    /// Positive = panels are approaching each other. Negative = separating.
    public let closingSpeed: CGFloat
    /// Seconds since last tick.
    public let dt: CGFloat
    /// The scope's merge radius, for reference.
    public let scopeRadius: CGFloat
}

// MARK: - Result Types

/// Additive style modifications from an interaction effect.
/// `nil` fields mean no change. Values are additive across multiple effects.
public struct FacetModification: Codable, Sendable, Equatable {
    public var frostDelta: CGFloat?
    public var refractionDelta: CGFloat?
    public var dispersionDelta: CGFloat?

    public init(
        frostDelta: CGFloat? = nil,
        refractionDelta: CGFloat? = nil,
        dispersionDelta: CGFloat? = nil
    ) {
        self.frostDelta = frostDelta
        self.refractionDelta = refractionDelta
        self.dispersionDelta = dispersionDelta
    }

    public static let none = FacetModification()

    /// Merges two modifications by adding their deltas.
    public func merged(with other: FacetModification) -> FacetModification {
        FacetModification(
            frostDelta: addOptional(frostDelta, other.frostDelta),
            refractionDelta: addOptional(refractionDelta, other.refractionDelta),
            dispersionDelta: addOptional(dispersionDelta, other.dispersionDelta)
        )
    }

    private func addOptional(_ a: CGFloat?, _ b: CGFloat?) -> CGFloat? {
        switch (a, b) {
        case let (a?, b?): return a + b
        case let (a?, nil): return a
        case let (nil, b?): return b
        case (nil, nil): return nil
        }
    }
}

/// The result of evaluating one interaction effect on a pair of children.
public struct ResonanceResult: Sendable {
    /// Continuous force applied to child A each frame. Child B gets the negated force (Newton's 3rd law).
    /// Integrated as `velocity += force * dt`.
    public var forceOnA: CGVector
    /// Instantaneous velocity impulse on child A. Child B gets the negated impulse.
    /// Applied directly as `velocity += impulse` (no dt scaling), so it punches through damping.
    /// Use for short, sharp interactions like bounce/collision.
    public var impulseOnA: CGVector
    /// Style modification for child A.
    public var styleModA: FacetModification
    /// Style modification for child B.
    public var styleModB: FacetModification
    /// If true, A's zIndex jumps above B (spring-animated).
    public var promoteA: Bool

    public init(
        forceOnA: CGVector = .zero,
        impulseOnA: CGVector = .zero,
        styleModA: FacetModification = .none,
        styleModB: FacetModification = .none,
        promoteA: Bool = false
    ) {
        self.forceOnA = forceOnA
        self.impulseOnA = impulseOnA
        self.styleModA = styleModA
        self.styleModB = styleModB
        self.promoteA = promoteA
    }

    public static let none = ResonanceResult()
}

// MARK: - Protocol

/// A pairwise interaction effect between glass panels.
///
/// Conform to this protocol to create custom glass-to-glass interactions.
/// Effects are evaluated for each pair of children within `interactionRange`.
public protocol ResonanceEffect {
    /// Maximum distance (center-to-center) at which this effect is evaluated.
    /// Pairs beyond this distance are skipped.
    var interactionRange: CGFloat { get }

    /// Evaluate this effect for a pair of children.
    func evaluate(context: ResonanceContext) -> ResonanceResult
}

// MARK: - Type-Erased Wrapper

/// Type-erased wrapper for `ResonanceEffect`, enabling dot-syntax API.
///
/// ```swift
/// ConfluenceScope(effects: [.springAttract(), .autoFrost()]) { ... }
/// ```
public struct AnyResonanceEffect: ResonanceEffect {
    private let _range: CGFloat
    private let _evaluate: (ResonanceContext) -> ResonanceResult

    public var interactionRange: CGFloat { _range }

    public func evaluate(context: ResonanceContext) -> ResonanceResult {
        _evaluate(context)
    }

    /// Wraps any `ResonanceEffect` in a type-erased container.
    public init<E: ResonanceEffect>(_ effect: E) {
        _range = effect.interactionRange
        _evaluate = effect.evaluate
    }

    /// Creates a type-erased effect from closures.
    public init(
        range: CGFloat,
        evaluate: @escaping (ResonanceContext) -> ResonanceResult
    ) {
        _range = range
        _evaluate = evaluate
    }
}
