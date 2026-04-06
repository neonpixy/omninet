// OneEuroFilter.swift
// CrystalKit
//
// Minimal 1D One Euro Filter — the standard algorithm for low-latency
// pointer smoothing. Adapts cutoff frequency based on signal speed:
// smooth when still, responsive when moving.
//
// Reference: Casiez et al. 2012, "1€ Filter: A Simple Speed-based
// Low-pass Filter for Noisy Input in Interactive Systems."

struct OneEuroFilterState: Sendable {
    var prevRaw: Double = 0
    var prevFiltered: Double = 0
    var prevDxFiltered: Double = 0
    var initialized = false

    /// Low-pass filter helper: alpha from cutoff frequency and sample rate.
    private static func alpha(cutoff: Double, dt: Double) -> Double {
        let tau = 1.0 / (2.0 * .pi * cutoff)
        return 1.0 / (1.0 + tau / dt)
    }

    mutating func filter(
        _ value: Double,
        dt: Double,
        minCutoff: Double,
        beta: Double,
        dCutoff: Double = 1.0
    ) -> Double {
        guard initialized else {
            prevRaw = value
            prevFiltered = value
            prevDxFiltered = 0
            initialized = true
            return value
        }

        // Estimate derivative (speed)
        let dx = (value - prevRaw) / dt

        // Low-pass filter the derivative to reduce noise
        let alphaD = Self.alpha(cutoff: dCutoff, dt: dt)
        let dxFiltered = prevDxFiltered + alphaD * (dx - prevDxFiltered)

        // Adaptive cutoff: faster movement → higher cutoff → less lag
        let cutoff = minCutoff + beta * abs(dxFiltered)
        let alphaVal = Self.alpha(cutoff: cutoff, dt: dt)

        // Low-pass filter the value
        let filtered = prevFiltered + alphaVal * (value - prevFiltered)

        prevRaw = value
        prevFiltered = filtered
        prevDxFiltered = dxFiltered

        return filtered
    }
}
