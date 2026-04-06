// GleamHoverTracker.swift
// CrystalKit
//
// iPad-only: stores the current Apple Pencil / trackpad hover position
// in screen coordinates so all glass views can read it each render frame.
//
// The hover events are captured at the SwiftUI level via
// `.gleamTracking()` using `.onContinuousHover(coordinateSpace: .global)`.
// This avoids UIKit hit-testing issues where non-interactive Metal views
// block gesture recognizer delivery.
//
// iPhone uses gyroscope tilt instead (GleamTiltTracker).
// macOS uses NSEvent mouse monitoring in the NSView directly.

#if os(iOS)
import UIKit
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "HoverTracker")

@MainActor
final class GleamHoverTracker {

    // MARK: - Shared Instance

    static let shared = GleamHoverTracker()

    /// Screen-coordinate hover position (nil when no hover is active).
    /// Glass views read this in their render loop.
    nonisolated(unsafe) static var _sharedHoverPoint: CGPoint?

    // MARK: - Consumer Reference Counting

    /// Tracks how many glass views want hover data.
    /// When > 0, the SwiftUI hover modifier writes positions to `_sharedHoverPoint`.
    private var consumerCount = 0

    var isActive: Bool { consumerCount > 0 }

    func addConsumer() {
        consumerCount += 1
    }

    func removeConsumer() {
        consumerCount = max(0, consumerCount - 1)
        if consumerCount == 0 {
            GleamHoverTracker._sharedHoverPoint = nil
        }
    }

    private init() {}
}

#endif
