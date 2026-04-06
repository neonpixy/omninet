import Foundation
import simd

/// Temporal smoothing cache for glass resonance tinting.
///
/// Stores per-shape smoothed tint values that the **shader** blends with its
/// live probe result each frame. No GPU readback — the shader does all the
/// heavy lifting; this cache just provides the "previous frame" anchor.
///
/// The smoothing works because the shader outputs:
///   `displayedTint = mix(cpuTint, freshProbe, blendFactor)`
/// where `cpuTint` is a slowly-evolving anchor. Each frame the anchor nudges
/// toward a neutral value, keeping the tint stable while fresh probes add
/// subtle, low-amplitude updates.
///
/// **Usage:**
/// 1. Call ``tintAndAdvance(for:)`` each frame to get the current tint and
///    nudge it toward its target.
/// 2. Call ``seed(id:tint:)`` to provide an initial tint when a shape first
///    appears.
/// 3. Call ``invalidate(id:)`` or ``invalidateAll()`` on shape deletion or
///    document change.
@MainActor
public final class ResonanceTintCache {

    private struct Entry {
        var tint: SIMD3<Float>
    }

    private var entries: [UUID: Entry] = [:]

    // MARK: - Brilliance per-light tint smoothing

    private struct BrillianceEntry {
        var tints: [SIMD3<Float>]  // Up to 4 per-light tints
    }

    private var brillianceEntries: [UUID: BrillianceEntry] = [:]

    public init() {}

    /// Returns the current smoothed tint for the shape, or `nil` if unseeded.
    public func smoothedTint(for id: UUID) -> SIMD3<Float>? {
        entries[id]?.tint
    }

    /// Seed an initial tint for a shape (typically mid-gray).
    public func seed(id: UUID, tint: SIMD3<Float> = SIMD3<Float>(repeating: 0.5)) {
        if entries[id] == nil {
            entries[id] = Entry(tint: tint)
        }
    }

    /// Update the stored tint toward a target. Call once per shape per frame.
    /// `target` should be the caller's best estimate of the fresh tint
    /// (e.g. mid-gray, or the last known value). The blend factor controls
    /// how fast it converges.
    public func nudge(id: UUID, toward target: SIMD3<Float>, factor: Float = 0.02) {
        if var entry = entries[id] {
            entry.tint = entry.tint + (target - entry.tint) * factor
            entries[id] = entry
        }
    }

    // MARK: - Brilliance Per-Light Tints

    /// Seed brilliance tints for a shape (all white initially).
    /// Call once when Brilliance lights first appear on a glass node.
    public func seedBrilliance(id: UUID, count: Int) {
        if brillianceEntries[id] == nil {
            brillianceEntries[id] = BrillianceEntry(
                tints: Array(repeating: SIMD3<Float>(repeating: 1.0), count: max(count, 4))
            )
        }
    }

    /// Nudge each light's tint toward its sampled canvas color.
    /// Factor of 0.15 → ~7-frame convergence. Fast enough to follow light
    /// movement, smooth enough to avoid flicker.
    public func nudgeBrillianceTints(
        id: UUID,
        toward targets: [SIMD3<Float>],
        factor: Float = 0.15
    ) {
        guard var entry = brillianceEntries[id] else { return }
        for i in entry.tints.indices {
            let target = i < targets.count ? targets[i] : SIMD3(repeating: 1.0)
            entry.tints[i] = entry.tints[i] + (target - entry.tints[i]) * factor
        }
        brillianceEntries[id] = entry
    }

    /// Returns the smoothed per-light tints, or `nil` if unseeded.
    public func smoothedBrillianceTints(for id: UUID) -> [SIMD3<Float>]? {
        brillianceEntries[id]?.tints
    }

    /// Remove the cached tint for a specific shape.
    public func invalidate(id: UUID) {
        entries.removeValue(forKey: id)
        brillianceEntries.removeValue(forKey: id)
    }

    /// Clear all cached tints.
    public func invalidateAll() {
        entries.removeAll()
        brillianceEntries.removeAll()
    }
}
