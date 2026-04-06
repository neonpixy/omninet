import Foundation
import Metal
import os

/// Caches blurred background textures for glass shapes to prevent refraction
/// flattening when shapes partially exit the viewport.
///
/// When a glass shape's background capture region gets clipped by the screen edge,
/// the shader receives a truncated texture and refraction collapses near the boundary.
/// This cache stores the best (largest) capture per shape and serves it back when the
/// current frame's capture is smaller (more clipped), keeping refraction intact.
///
/// **Cache strategy:**
/// - Every frame, the caller captures the current (possibly clipped) background.
/// - If the cache holds a larger texture for that shape at the same zoom level, use it.
/// - Otherwise, update the cache with the fresh capture.
/// - Zoom changes invalidate entries (resolution mismatch).
@MainActor
public final class HolodeckBackdropCache {

    private static let logger = Logger(subsystem: "com.crystalkit", category: "HolodeckBackdropCache")

    /// A cached background capture for a single glass shape.
    private struct Entry {
        let blurred: MTLTexture
        let width: Int
        let height: Int
        let regionX: Int
        let regionY: Int
        let zoom: Float

        var area: Int { width * height }
    }

    private var entries: [UUID: Entry] = [:]

    public init() {}

    /// Look up a cached background texture for a glass shape.
    ///
    /// Returns the cached texture and its original capture geometry if the cache
    /// holds a larger (less clipped) capture than the current frame. Returns `nil`
    /// if no suitable cache entry exists — the caller should use the current capture
    /// and call ``update(id:blurred:width:height:regionX:regionY:zoom:)`` afterward.
    ///
    /// - Parameters:
    ///   - id: The shape's unique identifier.
    ///   - currentArea: The area (width × height) of the current frame's capture.
    ///   - zoom: The current canvas zoom level.
    /// - Returns: The cached texture and geometry, or `nil`.
    public func lookup(
        id: UUID,
        currentArea: Int,
        zoom: Float
    ) -> (texture: MTLTexture, width: Int, height: Int, regionX: Int, regionY: Int)? {
        // Invalidate if zoom changed (resolution mismatch)
        if let entry = entries[id], abs(entry.zoom - zoom) > 0.01 {
            entries.removeValue(forKey: id)
            return nil
        }

        guard let entry = entries[id], entry.area > currentArea else {
            return nil
        }

        return (
            texture: entry.blurred,
            width: entry.width,
            height: entry.height,
            regionX: entry.regionX,
            regionY: entry.regionY
        )
    }

    /// Store or update a background capture for a shape.
    ///
    /// Call this after capturing a fresh background texture. The cache only keeps
    /// the entry if it's at least as large as any existing cached version.
    ///
    /// - Parameters:
    ///   - id: The shape's unique identifier.
    ///   - blurred: The blurred background texture.
    ///   - width: Capture region width in pixels.
    ///   - height: Capture region height in pixels.
    ///   - regionX: Capture region origin X in pixels.
    ///   - regionY: Capture region origin Y in pixels.
    ///   - zoom: The current canvas zoom level.
    public func update(
        id: UUID,
        blurred: MTLTexture,
        width: Int,
        height: Int,
        regionX: Int,
        regionY: Int,
        zoom: Float
    ) {
        entries[id] = Entry(
            blurred: blurred,
            width: width,
            height: height,
            regionX: regionX,
            regionY: regionY,
            zoom: zoom
        )
    }

    /// Remove the cached entry for a specific shape.
    public func invalidate(id: UUID) {
        entries.removeValue(forKey: id)
    }

    /// Clear all cached entries. Call on document changes or full invalidation.
    public func invalidateAll() {
        entries.removeAll()
    }
}
