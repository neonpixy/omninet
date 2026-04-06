// ResonanceEngine.swift
// CrystalKit
//
// Physics evaluation loop for glass-to-glass interactions.
// Stores per-child velocity, offset, and z-order. Called from
// TimelineView(.animation) each frame.

import SwiftUI
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "InteractionEngine")

// MARK: - Physics State

struct InteractionPhysicsState {
    var velocity: CGVector = .zero
    var offset: CGVector = .zero
    var zIndex: Double = 0
    /// Sum of recent acceleration magnitudes for jiggle detection.
    var jiggleAccumulator: CGFloat = 0
    /// Previous frame's position for drag detection.
    var previousCenter: CGPoint?
}

// MARK: - Engine

@MainActor @Observable
final class ResonanceEngine {

    // MARK: Published Output

    /// Per-child offset from physics forces (child ID → CGSize).
    private(set) var offsetMap: [String: CGSize] = [:]

    /// Per-child style modifications (child ID → FacetModification).
    private(set) var styleModMap: [String: FacetModification] = [:]

    /// Per-child z-index for rendering order (child ID → Double).
    private(set) var zIndexMap: [String: Double] = [:]

    /// True when all velocities are below threshold and no style mods are active.
    private(set) var isAtRest: Bool = true

    // MARK: Internal State

    private var states: [String: InteractionPhysicsState] = [:]
    private var lastTickDate: Date?

    // MARK: Constants

    private let damping: CGFloat = 4.0
    private let returnStiffness: CGFloat = 0.0 // disabled — damping + snap-to-zero handle settling
    private let maxSpeed: CGFloat = 800.0
    private let maxDt: CGFloat = 0.033 // 33ms cap
    private let restThreshold: CGFloat = 0.5 // pt/s
    private let offsetRestThreshold: CGFloat = 0.1 // pts
    private let jiggleThreshold: CGFloat = 800.0
    private let jiggleDecay: CGFloat = 0.85
    private let dragDetectionThreshold: CGFloat = 2.0 // pts per frame

    // MARK: - Tick

    func tick(
        children: [ConfluenceChildInfo],
        effects: [AnyResonanceEffect],
        scopeRadius: CGFloat,
        now: Date
    ) {
        guard !children.isEmpty, !effects.isEmpty else {
            resetIfNeeded()
            return
        }

        // Compute dt
        let dt: CGFloat
        if let last = lastTickDate {
            dt = min(CGFloat(now.timeIntervalSince(last)), maxDt)
        } else {
            dt = 0.016 // first frame ~60fps
        }
        lastTickDate = now

        guard dt > 0 else { return }

        // Prune states for removed children
        let childIDs = Set(children.map(\.id))
        for key in states.keys where !childIDs.contains(key) {
            states.removeValue(forKey: key)
        }

        // Ensure states exist for all children
        for (index, child) in children.enumerated() {
            if states[child.id] == nil {
                states[child.id] = InteractionPhysicsState(zIndex: Double(index))
            }
        }

        // 1. Build snapshots with current physics offsets applied
        let snapshots = buildSnapshots(from: children)

        // 2. Detect dragging
        var draggingIDs: Set<String> = []
        for child in children {
            let state = states[child.id]!
            let currentCenter = CGPoint(
                x: child.frame.midX,
                y: child.frame.midY
            )
            if let prev = state.previousCenter {
                let predicted = CGPoint(
                    x: prev.x + state.velocity.dx * dt,
                    y: prev.y + state.velocity.dy * dt
                )
                let deviation = hypot(currentCenter.x - predicted.x, currentCenter.y - predicted.y)
                if deviation > dragDetectionThreshold {
                    draggingIDs.insert(child.id)
                }
            }
            states[child.id]!.previousCenter = currentCenter
        }

        // 3. Pairwise evaluation
        var accumulatedForces: [String: CGVector] = [:]
        var accumulatedImpulses: [String: CGVector] = [:]
        var accumulatedStyleMods: [String: FacetModification] = [:]
        var promotions: [String: Double] = [:] // childID → target zIndex

        for i in 0..<snapshots.count {
            for j in (i + 1)..<snapshots.count {
                let a = snapshots[i]
                let b = snapshots[j]

                // Ensure A is back (lower zIndex), B is front (higher zIndex)
                let (back, front): (ResonanceSnapshot, ResonanceSnapshot)
                if a.zIndex <= b.zIndex {
                    back = a; front = b
                } else {
                    back = b; front = a
                }

                let context = buildContext(
                    childA: back,
                    childB: front,
                    draggingIDs: draggingIDs,
                    dt: dt,
                    scopeRadius: scopeRadius
                )

                for effect in effects {
                    let effectRange = effect.interactionRange == .infinity
                        ? scopeRadius
                        : effect.interactionRange
                    if context.distance > effectRange { continue }

                    let result = effect.evaluate(context: context)

                    // Accumulate forces (Newton's 3rd law)
                    accumulatedForces[back.id, default: .zero] = addVectors(
                        accumulatedForces[back.id, default: .zero],
                        result.forceOnA
                    )
                    accumulatedForces[front.id, default: .zero] = addVectors(
                        accumulatedForces[front.id, default: .zero],
                        CGVector(dx: -result.forceOnA.dx, dy: -result.forceOnA.dy)
                    )

                    // Accumulate impulses (Newton's 3rd law)
                    accumulatedImpulses[back.id, default: .zero] = addVectors(
                        accumulatedImpulses[back.id, default: .zero],
                        result.impulseOnA
                    )
                    accumulatedImpulses[front.id, default: .zero] = addVectors(
                        accumulatedImpulses[front.id, default: .zero],
                        CGVector(dx: -result.impulseOnA.dx, dy: -result.impulseOnA.dy)
                    )

                    // Accumulate style mods
                    accumulatedStyleMods[back.id] = (accumulatedStyleMods[back.id] ?? .none)
                        .merged(with: result.styleModA)
                    accumulatedStyleMods[front.id] = (accumulatedStyleMods[front.id] ?? .none)
                        .merged(with: result.styleModB)

                    // Z-order promotion
                    if result.promoteA {
                        let targetZ = (states[front.id]?.zIndex ?? 0) + 1
                        promotions[back.id] = max(promotions[back.id] ?? 0, targetZ)
                    }
                }
            }
        }

        // 4. Apply promotions
        for (childID, targetZ) in promotions {
            states[childID]?.zIndex = targetZ
        }

        // 5. Integrate physics
        var allAtRest = true
        for child in children {
            guard var state = states[child.id] else { continue }

            let force = accumulatedForces[child.id] ?? .zero
            let impulse = accumulatedImpulses[child.id] ?? .zero

            // Track previous velocity for jiggle detection
            let prevVelocity = state.velocity

            // Return-to-origin spring: gently pulls offset back to zero.
            // Prevents unbounded drift and ensures panels settle at their
            // natural layout position when no interaction forces are active.
            let returnForceX = -returnStiffness * state.offset.dx
            let returnForceY = -returnStiffness * state.offset.dy

            // Semi-implicit Euler (continuous forces + return spring)
            state.velocity.dx += (force.dx + returnForceX) * dt
            state.velocity.dy += (force.dy + returnForceY) * dt

            // Impulses: direct velocity kick, no dt scaling.
            // Punches through damping for snappy collision response.
            state.velocity.dx += impulse.dx
            state.velocity.dy += impulse.dy

            // Exponential damping (frame-rate independent)
            let dampFactor = exp(-damping * dt)
            state.velocity.dx *= dampFactor
            state.velocity.dy *= dampFactor

            // Speed clamp
            let speed = hypot(state.velocity.dx, state.velocity.dy)
            if speed > maxSpeed {
                let scale = maxSpeed / speed
                state.velocity.dx *= scale
                state.velocity.dy *= scale
            }

            // Jiggle detection
            let accelMag = hypot(
                state.velocity.dx - prevVelocity.dx,
                state.velocity.dy - prevVelocity.dy
            ) / dt
            state.jiggleAccumulator = state.jiggleAccumulator * jiggleDecay + accelMag * dt

            // Integrate position
            state.offset.dx += state.velocity.dx * dt
            state.offset.dy += state.velocity.dy * dt

            // Snap to zero when offset and velocity are negligible.
            // Prevents perpetual micro-drift.
            let offsetMag = hypot(state.offset.dx, state.offset.dy)
            if speed < restThreshold && offsetMag < offsetRestThreshold {
                state.offset = .zero
                state.velocity = .zero
            }

            // Rest detection
            if speed > restThreshold || offsetMag > offsetRestThreshold {
                allAtRest = false
            }

            states[child.id] = state
        }

        // Check if style mods are active
        if !accumulatedStyleMods.isEmpty {
            for mod in accumulatedStyleMods.values {
                if mod.frostDelta != nil || mod.refractionDelta != nil || mod.dispersionDelta != nil {
                    allAtRest = false
                    break
                }
            }
        }

        isAtRest = allAtRest

        // 6. Publish
        var newOffsetMap: [String: CGSize] = [:]
        var newZIndexMap: [String: Double] = [:]
        for (id, state) in states {
            if abs(state.offset.dx) > 0.01 || abs(state.offset.dy) > 0.01 {
                newOffsetMap[id] = CGSize(width: state.offset.dx, height: state.offset.dy)
            }
            newZIndexMap[id] = state.zIndex
        }
        offsetMap = newOffsetMap
        styleModMap = accumulatedStyleMods
        zIndexMap = newZIndexMap
    }

    // MARK: - Helpers

    private func resetIfNeeded() {
        if !states.isEmpty {
            states.removeAll()
            offsetMap = [:]
            styleModMap = [:]
            zIndexMap = [:]
            isAtRest = true
            lastTickDate = nil
        }
    }

    private func buildSnapshots(from children: [ConfluenceChildInfo]) -> [ResonanceSnapshot] {
        // Use child.frame directly — it already includes the interaction
        // offset from the previous frame (via .offset() being reflected
        // in the coordinate space measurement). No need to add engine
        // offset here; doing so would double-count.
        children.enumerated().map { index, child in
            let state = states[child.id]
            return ResonanceSnapshot(
                id: child.id,
                frame: child.frame,
                style: child.style,
                crystallized: child.crystallized,
                zIndex: Int(state?.zIndex ?? Double(index)),
                isDragging: false
            )
        }
    }

    private func buildContext(
        childA: ResonanceSnapshot,
        childB: ResonanceSnapshot,
        draggingIDs: Set<String>,
        dt: CGFloat,
        scopeRadius: CGFloat
    ) -> ResonanceContext {
        let centerA = CGPoint(x: childA.frame.midX, y: childA.frame.midY)
        let centerB = CGPoint(x: childB.frame.midX, y: childB.frame.midY)

        let dx = centerB.x - centerA.x
        let dy = centerB.y - centerA.y
        let dist = hypot(dx, dy)

        let direction: CGVector
        if dist > 0.001 {
            direction = CGVector(dx: dx / dist, dy: dy / dist)
        } else {
            direction = .zero
        }

        // Compute normalized overlap
        let intersection = childA.frame.intersection(childB.frame)
        let overlap: CGFloat
        if intersection.isNull || intersection.isEmpty {
            overlap = 0
        } else {
            let intersectionArea = intersection.width * intersection.height
            let minArea = min(
                childA.frame.width * childA.frame.height,
                childB.frame.width * childB.frame.height
            )
            overlap = minArea > 0 ? min(intersectionArea / minArea, 1) : 0
        }

        // Rebuild snapshots with dragging info and jiggle state
        let stateA = states[childA.id]
        let isDraggingA = draggingIDs.contains(childA.id)
            || (stateA?.jiggleAccumulator ?? 0) > jiggleThreshold
        let snapshotA = ResonanceSnapshot(
            id: childA.id,
            frame: childA.frame,
            style: childA.style,
            crystallized: childA.crystallized,
            zIndex: childA.zIndex,
            isDragging: isDraggingA
        )

        let stateB = states[childB.id]
        let isDraggingB = draggingIDs.contains(childB.id)
            || (stateB?.jiggleAccumulator ?? 0) > jiggleThreshold
        let snapshotB = ResonanceSnapshot(
            id: childB.id,
            frame: childB.frame,
            style: childB.style,
            crystallized: childB.crystallized,
            zIndex: childB.zIndex,
            isDragging: isDraggingB
        )

        // Closing speed: relative velocity projected onto A→B axis.
        // Positive = approaching, negative = separating.
        let velA = stateA?.velocity ?? .zero
        let velB = stateB?.velocity ?? .zero
        let relVelX = velA.dx - velB.dx  // A's velocity relative to B
        // Project onto A→B direction: dot(relVel, direction)
        // Positive dot = A moving toward B = closing
        let closingSpeed = relVelX * direction.dx + (velA.dy - velB.dy) * direction.dy

        return ResonanceContext(
            childA: snapshotA,
            childB: snapshotB,
            distance: dist,
            overlap: overlap,
            direction: direction,
            closingSpeed: closingSpeed,
            dt: dt,
            scopeRadius: scopeRadius
        )
    }

    private func addVectors(_ a: CGVector, _ b: CGVector) -> CGVector {
        CGVector(dx: a.dx + b.dx, dy: a.dy + b.dy)
    }
}
