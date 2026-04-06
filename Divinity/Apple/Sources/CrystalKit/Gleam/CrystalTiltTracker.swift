// GleamTiltTracker.swift
// CrystalKit
//
// iPhone-only: maps device gravity vector to a light source direction
// and combines tilt magnitude with screen brightness for light intensity.
// No permissions required. No baseline or warmup needed.
//
// Gravity vector approach:
//   Phone flat → gravity ≈ (0, 0, -1) → light centered, dim
//   Phone tilted → gravity.x/.y shift → light moves, intensity rises
//   Like tilting a real glass under a lamp.
//
// iPad uses hover/Apple Pencil tracking instead; macOS uses mouse.

#if os(iOS)
import CoreMotion
import UIKit
import QuartzCore
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "TiltTracker")

@MainActor
final class GleamTiltTracker {

    // MARK: - Shared Instance

    static let shared = GleamTiltTracker()

    /// Shared light direction that views poll each frame.
    /// Normalized to roughly -0.5…+0.5 per axis (0,0 = center).
    /// Derived from the device gravity vector — no baseline needed.
    /// `nil` when tilt tracking is inactive (e.g. on iPad).
    nonisolated(unsafe) static var _sharedLightDirection: CGPoint?

    /// Combined intensity from screen brightness and tilt magnitude.
    /// Range: 0.3 (flat + dark room) to 1.0 (tilted + bright screen).
    /// Glass is always somewhat visible even at minimum.
    nonisolated(unsafe) static var _sharedBrightnessIntensity: CGFloat = 0.7

    // MARK: - Consumer Reference Counting

    private var consumerCount = 0

    /// Call when a view using `.cursor()` light source appears.
    func addConsumer() {
        consumerCount += 1
        if consumerCount == 1 { start() }
    }

    /// Call when a view using `.cursor()` light source disappears.
    func removeConsumer() {
        consumerCount = max(0, consumerCount - 1)
        if consumerCount == 0 { stop() }
    }

    // MARK: - Configuration

    /// How much of the gravity vector maps to light direction.
    /// At 0.5, a 30° tilt moves the light halfway from center to edge.
    /// gravity.x/y range is ±1.0 (at 90°), so gain of 0.5 → ±0.5 output.
    private let directionGain: CGFloat = 1.0

    /// How much tilt magnitude boosts light intensity.
    /// 0 = tilt has no effect on brightness, 1 = full effect.
    /// The tilt component is added on top of the screen brightness base.
    private let tiltIntensityGain: CGFloat = 1.2

    /// One Euro Filter: smooth at rest, responsive when tilting.
    /// minCutoff: base cutoff frequency (Hz). Higher = less smoothing at rest.
    /// beta: speed coefficient. Higher = filter opens up faster for quick moves,
    ///       preventing the "lag then snap" effect.
    private let filterMinCutoff: Double = 1.2
    private let filterBeta: Double = 0.05

    // MARK: - Private State

    nonisolated(unsafe) private let motionManager = CMMotionManager()
    nonisolated(unsafe) private var displayLink: CADisplayLink?

    // One Euro Filters for smooth output (direction X, Y, and intensity).
    nonisolated(unsafe) private var filterX = OneEuroFilterState()
    nonisolated(unsafe) private var filterY = OneEuroFilterState()
    nonisolated(unsafe) private var filterIntensity = OneEuroFilterState()
    nonisolated(unsafe) private var lastFrameTime: CFTimeInterval = 0

    // Brightness polling counter (sample at ~4Hz, not 60Hz).
    nonisolated(unsafe) private var brightnessCounter = 0
    nonisolated(unsafe) private var screenBrightness: CGFloat = 0.7

    private init() {}

    // MARK: - Start / Stop

    private func start() {
        // Tilt tracking works on both iPhone and iPad.
        // On M2+ iPads with hover support, hover takes priority over tilt
        // in the cursorWorldPos() chain — tilt acts as the fallback.
        // On pre-M2 iPads (no hover hardware), tilt is the only input.

        filterX = OneEuroFilterState()
        filterY = OneEuroFilterState()
        filterIntensity = OneEuroFilterState()
        lastFrameTime = 0
        brightnessCounter = 0

        guard motionManager.isDeviceMotionAvailable else {
            logger.warning("Device motion not available — tilt tracking disabled")
            return
        }

        motionManager.deviceMotionUpdateInterval = 1.0 / 60.0
        motionManager.startDeviceMotionUpdates(using: .xArbitraryZVertical)

        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        displayLink = link

        // Seed brightness immediately.
        sampleScreenBrightness()

        logger.info("Tilt tracking started (gravity-based)")
    }

    private func stop() {
        displayLink?.invalidate()
        displayLink = nil
        motionManager.stopDeviceMotionUpdates()
        GleamTiltTracker._sharedLightDirection = nil
        logger.info("Tilt tracking stopped")
    }

    // MARK: - Frame Update

    @objc nonisolated private func tick() {
        guard let motion = motionManager.deviceMotion else { return }
        let g = motion.gravity  // (x, y, z) — which way "down" points relative to phone

        let now = CACurrentMediaTime()
        let dt: Double
        if lastFrameTime == 0 {
            dt = 1.0 / 60.0
        } else {
            dt = max(now - lastFrameTime, 1.0 / 120.0)
        }
        lastFrameTime = now

        // ── Direction ──
        // When held upright in portrait, gravity ≈ (0, -1, 0).
        //   gravity.x → left/right tilt (0 at rest, ±1 at 90°)
        //   gravity.z → forward/back tilt (0 when upright, ±1 when flat)
        //   gravity.y stays near -1 in portrait — useless for direction.
        //
        // Use atan2 for the direction angle — this wraps smoothly at extreme
        // tilts instead of clamping raw components (which snap to the sides
        // when one axis saturates and the other has noise).
        let tiltAngle = atan2(g.x, g.z)  // radians, 0 = pure forward tilt
        let tiltRadius = min(sqrt(g.x * g.x + g.z * g.z), 1.0)
        // Reconstruct smooth X/Y from angle + clamped radius.
        // Scale radius by directionGain to control sensitivity.
        let scaledR = min(tiltRadius * Double(directionGain), 0.5)
        let rawX = CGFloat(sin(tiltAngle)) * CGFloat(scaledR)
        let rawY = CGFloat(cos(tiltAngle)) * CGFloat(scaledR)

        let clampedX = max(-0.5, min(0.5, rawX))
        let clampedY = max(-0.5, min(0.5, rawY))

        // ── Tilt magnitude → intensity boost ──
        // Use x and z (the axes that actually move in portrait) for magnitude.
        // sqrt(x² + z²) = how far from upright neutral. Ranges 0 to ~1.
        // We use this to boost rim light intensity: more tilt = brighter highlight.
        let tiltMagnitude = min(sqrt(g.x * g.x + g.z * g.z), 1.0)

        // One Euro Filter everything for smooth output.
        let fx = filterX.filter(Double(clampedX), dt: dt,
                                minCutoff: filterMinCutoff, beta: filterBeta)
        let fy = filterY.filter(Double(clampedY), dt: dt,
                                minCutoff: filterMinCutoff, beta: filterBeta)
        let fIntensity = filterIntensity.filter(tiltMagnitude, dt: dt,
                                                minCutoff: 0.4, beta: 0.001)

        GleamTiltTracker._sharedLightDirection = CGPoint(x: fx, y: fy)

        // Combine screen brightness (base) with tilt magnitude (boost).
        // Floor of 0.3: glass is always visible even flat in a dark room.
        // Screen brightness contributes the base level.
        // Tilt magnitude adds on top, scaled by tiltIntensityGain.
        let base = 0.3 + 0.4 * pow(Double(screenBrightness), 0.8)
        let tiltBoost = CGFloat(fIntensity) * tiltIntensityGain * 0.3
        GleamTiltTracker._sharedBrightnessIntensity = min(CGFloat(base) + tiltBoost, 1.0)

        // Sample screen brightness at ~4Hz (every 15 frames). It changes slowly.
        brightnessCounter += 1
        if brightnessCounter % 15 == 0 {
            sampleScreenBrightness()
        }
    }

    // MARK: - Brightness

    nonisolated private func sampleScreenBrightness() {
        MainActor.assumeIsolated {
            screenBrightness = UIScreen.main.brightness  // 0…1
        }
    }
}

#endif
