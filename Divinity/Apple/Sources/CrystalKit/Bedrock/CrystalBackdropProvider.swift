// BedrockProvider.swift
// CrystalKit
//
// Public protocol that lets host apps supply a backdrop texture directly,
// bypassing CGWindowListCreateImage. This avoids screen-recording permission
// and the hide/flush/capture/restore flicker dance.

import Metal
import SwiftUI

/// A type that can provide the backdrop content behind a glass view as a Metal texture.
///
/// Implement this protocol in your app to supply the backdrop directly from your
/// own rendering pipeline (e.g. a Metal canvas). When set via the `.bedrock(_:)`
/// environment modifier, CrystalKit will call `backdropTexture(for:in:)` each frame
/// instead of using `CGWindowListCreateImage`.
///
/// Example:
/// ```swift
/// struct CanvasBackdropProvider: BedrockProvider {
///     let metalLayer: CAMetalLayer
///
///     func backdropTexture(for rect: CGRect, in window: NSWindow) -> MTLTexture? {
///         // Return the canvas texture cropped to rect
///     }
/// }
/// ```
@MainActor
public protocol BedrockProvider: AnyObject {
    /// Returns a Metal texture containing the backdrop content for the given region.
    ///
    /// - Parameters:
    ///   - rect: The region to capture, in window coordinates (points, bottom-left origin on macOS).
    ///   - scale: The backing scale factor (e.g. 2.0 for Retina).
    /// - Returns: A `.bgra8Unorm` texture, or nil if capture isn't available this frame.
    func backdropTexture(for rect: CGRect, scale: CGFloat) -> MTLTexture?

    /// Monotonically increasing counter incremented each time new content is available.
    /// Glass views use this to skip the blur pipeline when the backdrop hasn't changed.
    /// Defaults to 0 for providers that don't implement it (treated as always-dirty).
    var contentGeneration: UInt64 { get }
}

public extension BedrockProvider {
    var contentGeneration: UInt64 { 0 }
}

// MARK: - Environment Key

private struct BedrockProviderKey: EnvironmentKey {
    nonisolated(unsafe) static let defaultValue: (any BedrockProvider)? = nil
}

extension EnvironmentValues {
    /// The backdrop provider used by CrystalKit glass views.
    public var bedrockProvider: (any BedrockProvider)? {
        get { self[BedrockProviderKey.self] }
        set { self[BedrockProviderKey.self] = newValue }
    }
}

// MARK: - View Extension

extension View {
    /// Sets a custom backdrop provider for all CrystalKit glass views in this subtree.
    ///
    /// When set, glass views will call the provider each frame instead of using
    /// `CGWindowListCreateImage`, avoiding screen-recording permission and flicker.
    public func bedrock(_ provider: (any BedrockProvider)?) -> some View {
        environment(\.bedrockProvider, provider)
    }
}
