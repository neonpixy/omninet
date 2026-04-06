// CrystalInteractionBuiltins.swift
// CrystalKit
//
// Built-in interaction effects: SpringAttract, AutoFrost, Bounce.

import SwiftUI

// MARK: - Spring Attract

/// Attracts nearby glass panels with a spring force.
///
/// Force is proportional to `(1 - normalizedDist) * stiffness`.
/// When a dragged back-panel is jiggling while overlapping a front panel,
/// returns `promoteA: true` to pop it to the front.
struct SpringAttractEffect: ResonanceEffect {
    let strength: CGFloat
    let range: CGFloat

    var interactionRange: CGFloat { range }

    func evaluate(context: ResonanceContext) -> ResonanceResult {
        let effectiveRange = range.isInfinite ? context.scopeRadius : range
        let normalizedDist = min(context.distance / effectiveRange, 1)
        let stiffness: CGFloat = 500 + strength * 1500

        // Linear pull to snap panels together, easing off once merged.
        // Without the overlap reduction, panels that are already touching
        // get shoved harder — the opposite of what feels natural.
        let proximity = 1 - normalizedDist
        let overlapReduction = 1 - context.overlap * 0.85
        let forceMagnitude = proximity * stiffness * overlapReduction
        let forceOnA = CGVector(
            dx: context.direction.dx * forceMagnitude,
            dy: context.direction.dy * forceMagnitude
        )

        // Z-order promotion: if the back panel (A) is being dragged and
        // jiggling while overlapping the front panel (B), pop A to the front.
        let promoteA = context.childA.isDragging && context.overlap > 0.1

        return ResonanceResult(
            forceOnA: forceOnA,
            promoteA: promoteA
        )
    }
}

// MARK: - Auto Frost

/// Frosts the front panel when two glass panels overlap.
///
/// Z-aware: only the FRONT panel (childB, higher zIndex) receives frost.
/// The panel behind sees no frost change — you're looking through two layers
/// of glass only from the front panel's perspective.
struct AutoFrostEffect: ResonanceEffect {
    let intensity: CGFloat
    let range: CGFloat

    var interactionRange: CGFloat { range }

    func evaluate(context: ResonanceContext) -> ResonanceResult {
        let effectiveRange = range.isInfinite ? context.scopeRadius : range
        let normalizedDist = min(context.distance / effectiveRange, 1)
        let proximity = 1 - normalizedDist

        // Only frost the front panel (B).
        let frostDelta = intensity * proximity * 0.5

        return ResonanceResult(
            styleModB: FacetModification(frostDelta: frostDelta)
        )
    }
}

// MARK: - Bounce

/// Bounces overlapping glass panels apart with a velocity impulse.
///
/// When two panels overlap, an instantaneous velocity kick pushes them apart
/// along the center-to-center axis. The impulse scales with overlap depth —
/// grazing contact is a gentle tap, deep overlap is a hard shove.
/// No force at distance — only fires on contact.
struct BounceEffect: ResonanceEffect {
    let strength: CGFloat
    let range: CGFloat

    var interactionRange: CGFloat { range }

    func evaluate(context: ResonanceContext) -> ResonanceResult {
        // Only activate when frames actually overlap.
        guard context.overlap > 0 else { return .none }

        // Only fire when panels are approaching — not when already
        // separating. A small positive threshold prevents re-firing
        // when velocity has decayed to near-zero but panels still overlap
        // (which would cause shuddering).
        guard context.closingSpeed > -5 else { return .none }

        // Strong impulse: 300-800 pt/s depending on strength.
        // With damping=8, this produces 37-100pt displacement — enough
        // to visibly launch a panel out of overlap range.
        let baseKick: CGFloat = 500 + strength * 1000
        let impulseMagnitude = max(context.overlap, 0.3) * baseKick

        // Push A away from B (negative direction).
        let direction = context.direction
        let safeDirection: CGVector
        if abs(direction.dx) < 0.001 && abs(direction.dy) < 0.001 {
            safeDirection = CGVector(dx: 0, dy: -1)
        } else {
            safeDirection = direction
        }

        let impulseOnA = CGVector(
            dx: -safeDirection.dx * impulseMagnitude,
            dy: -safeDirection.dy * impulseMagnitude
        )

        return ResonanceResult(impulseOnA: impulseOnA)
    }
}

// MARK: - Promote On Drag

/// Promotes a panel's z-order when it is dragged over another panel.
///
/// No physics forces, no style modifications — this only adjusts z-index
/// so the dragged panel renders in front. Designed for `FacetScope` where
/// glass panels are crystallized (no SDF merge) but still need depth ordering.
struct PromoteOnDragEffect: ResonanceEffect {
    var interactionRange: CGFloat { .infinity }

    func evaluate(context: ResonanceContext) -> ResonanceResult {
        guard context.overlap > 0.05 else { return .none }
        let promoteA = context.childA.isDragging
        return ResonanceResult(promoteA: promoteA)
    }
}

// MARK: - Dot-Syntax Factories

extension AnyResonanceEffect {
    /// Spring attraction between nearby glass panels.
    ///
    /// - Parameters:
    ///   - strength: Force multiplier (0-1). Default 0.5.
    ///   - range: Maximum attraction distance. Defaults to the scope's merge radius.
    public static func springAttract(
        strength: CGFloat = 0.5,
        range: CGFloat? = nil
    ) -> AnyResonanceEffect {
        // range = nil means "use scope radius" — resolved at tick time.
        // We store a sentinel and let the engine substitute the scope radius.
        AnyResonanceEffect(SpringAttractEffect(
            strength: strength,
            range: range ?? .infinity
        ))
    }

    /// Bounce panels apart on overlap.
    ///
    /// Panels that collide get pushed away from each other with an impulse
    /// proportional to overlap depth. No force at distance — only on contact.
    ///
    /// - Parameters:
    ///   - strength: Force multiplier (0-1). Default 0.5.
    ///   - range: Maximum detection distance. Defaults to the scope's merge radius.
    public static func bounce(
        strength: CGFloat = 0.5,
        range: CGFloat? = nil
    ) -> AnyResonanceEffect {
        AnyResonanceEffect(BounceEffect(
            strength: strength,
            range: range ?? .infinity
        ))
    }

    /// Promotes the dragged panel's z-order above the panel it overlaps.
    ///
    /// Lightweight effect: no forces, no style changes — only depth promotion.
    /// Pair with multi-pass rendering in `FacetScope` for glass-on-glass depth.
    public static func promoteOnDrag() -> AnyResonanceEffect {
        AnyResonanceEffect(PromoteOnDragEffect())
    }

    /// Auto-frost the front panel when two glass panels overlap.
    ///
    /// - Parameters:
    ///   - intensity: Frost strength multiplier (0-1). Default 0.5.
    ///   - range: Maximum distance for the frost effect. Defaults to the scope's merge radius.
    public static func autoFrost(
        intensity: CGFloat = 0.5,
        range: CGFloat? = nil
    ) -> AnyResonanceEffect {
        AnyResonanceEffect(AutoFrostEffect(
            intensity: intensity,
            range: range ?? .infinity
        ))
    }
}
