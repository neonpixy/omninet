// BedrockCapture.swift
// CrystalKit
//
// Platform-specific backdrop capture: captures the screen content behind a
// glass view into a Metal texture for the blur/refraction pipeline.

import Metal
import CoreGraphics
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "BedrockCapture")

// MARK: - macOS Backdrop Capture

#if os(macOS)
import AppKit

/// Backdrop capture strategy for macOS.
///
/// - `layerRender`: Uses `CALayer.render(in:)`. Synchronous, no feedback loop,
///   no permissions needed. Handles all Core Animation content (SwiftUI images,
///   gradients, text, shapes). Cannot capture CAMetalLayer drawables.
///   **Default and recommended for most apps.**
///
/// - `windowServer`: Uses `CGWindowListCreateImage`. Captures everything including
///   Metal drawables, but reads from the window server's composited frame, which
///   introduces a one-frame latency and requires careful layer-hiding to avoid
///   feedback loops. Use only if you have Metal content behind glass AND cannot
///   use a `BedrockProvider`.
enum BedrockCaptureStrategy: Sendable {
    case layerRender
    case windowServer
}

@MainActor
final class BedrockCapture {

    private let device: MTLDevice
    private var captureTexture: MTLTexture?
    private var captureSize: (Int, Int) = (0, 0)
    var strategy: BedrockCaptureStrategy = .layerRender

    /// The backing scale factor of the last capture (pixels per point).
    private(set) var captureScale: CGFloat = 2.0

    init(device: MTLDevice) {
        self.device = device
    }

    /// Captures the **full window** content into a Metal texture.
    ///
    /// Returns the full-window texture. The caller uses the glass view's current
    /// frame-in-window (in points, scaled by `captureScale`) to crop the relevant
    /// sub-region on each render frame. This avoids recapturing on drag/scroll —
    /// only the crop region shifts.
    ///
    /// > Note: The `.windowServer` strategy reads from the compositor's already-
    /// > composited buffer, which may include the glass effect from the previous
    /// > frame. For ghost-free results, use a `BedrockProvider` instead.
    func captureFullWindow(for view: NSView) -> MTLTexture? {
        switch strategy {
        case .layerRender:   captureFullWindowViaLayerRender(for: view)
        case .windowServer:  captureFullWindowViaWindowServer(for: view)
        }
    }

    /// Convenience: captures just the view-sized region behind the given view.
    /// Used by ConfluenceView and other callers that don't need full-window caching.
    ///
    /// Uses `CALayer.render(in:)` with the view hidden so the capture excludes the
    /// view's own Metal output. This prevents the feedback loop that occurs when
    /// `CGWindowListCreateImage` captures the composited window including the
    /// Metal layer's previous frame.
    func capture(behind view: NSView) -> MTLTexture? {
        guard let window = view.window,
              let contentView = window.contentView else { return nil }

        let scale = window.backingScaleFactor
        captureScale = scale

        // Convert to contentView coordinates — contentView.layer.render uses this
        // coordinate space, which may differ from window coordinates by the title bar.
        let viewFrame = view.convert(view.bounds, to: contentView)
        let pixelWidth = Int(viewFrame.width * scale)
        let pixelHeight = Int(viewFrame.height * scale)
        guard pixelWidth > 0, pixelHeight > 0 else { return nil }

        ensureTexture(width: pixelWidth, height: pixelHeight)
        guard let texture = captureTexture else { return nil }

        let bytesPerRow = pixelWidth * 4
        guard let context = CGContext(
            data: nil,
            width: pixelWidth,
            height: pixelHeight,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else { return nil }

        // Hide the view so CALayer.render skips the Metal layer's own output.
        let wasHidden = view.isHidden
        view.isHidden = true

        // CALayer.render uses bottom-left origin; flip to match Metal's top-left.
        context.translateBy(x: 0, y: CGFloat(pixelHeight))
        context.scaleBy(x: scale, y: -scale)
        // Translate so only the view-sized region is captured.
        context.translateBy(x: -viewFrame.origin.x, y: -viewFrame.origin.y)
        contentView.layer?.render(in: context)

        view.isHidden = wasHidden

        guard let data = context.data else { return nil }
        texture.replace(
            region: MTLRegionMake2D(0, 0, pixelWidth, pixelHeight),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: bytesPerRow
        )

        return texture
    }

    // MARK: - CALayer.render

    /// Synchronous full-window capture via Core Animation. Renders the window's
    /// layer tree into a CGContext. No window server round-trip.
    ///
    /// Note: Cannot capture CAMetalLayer drawables (renders black for Metal content).
    /// Use a `BedrockProvider` for apps with Metal backdrops behind glass.
    private func captureFullWindowViaLayerRender(for view: NSView) -> MTLTexture? {
        guard let window = view.window,
              let contentView = window.contentView else { return nil }

        let scale = window.backingScaleFactor
        captureScale = scale
        let pixelWidth = Int(contentView.bounds.width * scale)
        let pixelHeight = Int(contentView.bounds.height * scale)

        guard pixelWidth > 0, pixelHeight > 0 else { return nil }

        ensureTexture(width: pixelWidth, height: pixelHeight)
        guard let texture = captureTexture else { return nil }

        let bytesPerRow = pixelWidth * 4
        guard let context = CGContext(
            data: nil,
            width: pixelWidth,
            height: pixelHeight,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            logger.error("Failed to create CGContext for full-window backdrop capture")
            return nil
        }

        context.translateBy(x: 0, y: CGFloat(pixelHeight))
        context.scaleBy(x: scale, y: -scale)
        contentView.layer?.render(in: context)

        guard let data = context.data else { return nil }
        texture.replace(
            region: MTLRegionMake2D(0, 0, pixelWidth, pixelHeight),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: bytesPerRow
        )

        return texture
    }

    // MARK: - CGWindowListCreateImage

    /// Full-window capture via the window server. Captures the entire window
    /// including Metal layer drawables. The capture may include glass effects
    /// from the previous frame (the WindowServer compositor is async), which
    /// can produce a faint "ghost". For ghost-free results, use a
    /// `BedrockProvider` instead.
    private func captureFullWindowViaWindowServer(for view: NSView) -> MTLTexture? {
        // CGWindowListCreateImage was removed in macOS 26. This strategy is dead —
        // use ImageBedrockProvider for zero-ghost backdrop capture, or the default
        // .layerRender strategy for non-Metal backdrops.
        return nil
    }

    // MARK: - Texture Pool

    private func ensureTexture(width: Int, height: Int) {
        guard (width, height) != captureSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead]
        desc.storageMode = .managed

        captureTexture = device.makeTexture(descriptor: desc)
        captureTexture?.label = "CrystalKit Backdrop"
        captureSize = (width, height)
    }
}

#elseif os(iOS)
import UIKit

@MainActor
final class BedrockCapture {

    private let device: MTLDevice
    private var captureTexture: MTLTexture?
    private var captureSize: (Int, Int) = (0, 0)

    init(device: MTLDevice) {
        self.device = device
    }

    /// Captures the window content behind the given view into a Metal texture.
    ///
    /// Uses `CALayer.render(in:)` rather than `drawHierarchy(afterScreenUpdates:)`.
    /// Layer rendering is synchronous — the hidden state takes effect immediately,
    /// so there's no feedback loop from capturing the glass view's own output.
    /// Trade-off: can't capture CAMetalLayer drawables, but SwiftUI backgrounds
    /// (gradients, images, text) are all Core Animation and render correctly.
    ///
    /// - Parameters:
    ///   - view: The glass view to capture behind.
    ///   - gooTexture: Optional goo output texture to composite on top. On iOS,
    ///     `CALayer.render(in:)` can't see Metal layers, so this fills the gap.
    ///   - gooFrame: The goo scope view's frame in window coordinates (points).
    ///     Required when `gooTexture` is provided.
    ///   - captureScale: Pixel scale to use for the capture. Defaults to screen scale.
    ///     Pass a lower value (e.g. `screen.scale * 0.5`) for high-frost glass where
    ///     blur destroys fine detail, reducing the capture pixel count significantly.
    func capture(
        behind view: UIView,
        gooTexture: MTLTexture? = nil,
        gooFrame: CGRect = .zero,
        captureScale: CGFloat? = nil
    ) -> MTLTexture? {
        guard let window = view.window else { return nil }

        let viewFrameInWindow = view.convert(view.bounds, to: window)
        let scale = captureScale ?? window.screen.scale
        let pixelWidth = Int(viewFrameInWindow.width * scale)
        let pixelHeight = Int(viewFrameInWindow.height * scale)

        guard pixelWidth > 0, pixelHeight > 0 else { return nil }

        ensureTexture(width: pixelWidth, height: pixelHeight)
        guard let texture = captureTexture else { return nil }

        // Render directly into a BGRA context at pixel resolution.
        // Hide the glass view first — CALayer.render sees this immediately.
        let bytesPerRow = pixelWidth * 4
        guard let context = CGContext(
            data: nil,
            width: pixelWidth,
            height: pixelHeight,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else { return nil }

        let wasHidden = view.isHidden
        view.isHidden = true

        // CALayer.render draws in the layer's coordinate system (top-left origin)
        // but CGContext uses bottom-left origin. Flip vertically so the captured
        // image is right-side-up when uploaded to the Metal texture.
        context.translateBy(x: 0, y: CGFloat(pixelHeight))
        context.scaleBy(x: scale, y: -scale)
        context.translateBy(x: -viewFrameInWindow.origin.x, y: -viewFrameInWindow.origin.y)
        window.layer.render(in: context)

        view.isHidden = wasHidden

        guard let data = context.data else { return nil }

        // Composite goo output on top if provided. CALayer.render can't capture
        // Metal layer content, so we blend the goo texture into the captured backdrop.
        // This uses a CPU blend but is only hit when goo is present, and only on
        // dirty frames (not every display link tick).
        if let gooTex = gooTexture, gooFrame.width > 0, gooFrame.height > 0 {
            compositeGooTexture(
                gooTex, gooFrame: gooFrame,
                into: data, viewFrame: viewFrameInWindow,
                pixelWidth: pixelWidth, pixelHeight: pixelHeight,
                scale: scale
            )
        }

        texture.replace(
            region: MTLRegionMake2D(0, 0, pixelWidth, pixelHeight),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: bytesPerRow
        )

        return texture
    }

    /// Alpha-composites the goo output texture into the captured backdrop buffer.
    ///
    /// Both the goo texture and the backdrop context use BGRA premultiplied-alpha.
    /// The goo texture covers the full scope; we sample only the region that
    /// overlaps with the standalone glass view.
    private func compositeGooTexture(
        _ gooTex: MTLTexture,
        gooFrame: CGRect,
        into dstBytes: UnsafeMutableRawPointer,
        viewFrame: CGRect,
        pixelWidth: Int,
        pixelHeight: Int,
        scale: CGFloat
    ) {
        // Compute overlap in window-coordinate points.
        let overlap = viewFrame.intersection(gooFrame)
        guard !overlap.isNull, overlap.width > 0, overlap.height > 0 else { return }

        let gooTexW = gooTex.width
        let gooTexH = gooTex.height
        let gooPtW = gooFrame.width
        let gooPtH = gooFrame.height
        guard gooPtW > 0, gooPtH > 0 else { return }

        // Scale factors: goo texture pixels per point.
        let gooScaleX = CGFloat(gooTexW) / gooPtW
        let gooScaleY = CGFloat(gooTexH) / gooPtH

        // Source region in goo texture pixels.
        let srcX = Int((overlap.minX - gooFrame.minX) * gooScaleX)
        let srcY = Int((overlap.minY - gooFrame.minY) * gooScaleY)
        let srcW = Int(overlap.width * gooScaleX)
        let srcH = Int(overlap.height * gooScaleY)
        guard srcW > 0, srcH > 0 else { return }

        // Destination region in backdrop pixels.
        let dstX = Int((overlap.minX - viewFrame.minX) * scale)
        let dstY = Int((overlap.minY - viewFrame.minY) * scale)
        let dstW = min(srcW, pixelWidth - dstX)
        let dstH = min(srcH, pixelHeight - dstY)
        guard dstW > 0, dstH > 0 else { return }

        // Read goo pixels for the overlap region.
        let gooBytesPerRow = srcW * 4
        var gooPixels = [UInt8](repeating: 0, count: gooBytesPerRow * srcH)
        gooTex.getBytes(
            &gooPixels,
            bytesPerRow: gooBytesPerRow,
            from: MTLRegionMake2D(srcX, srcY, srcW, srcH),
            mipmapLevel: 0
        )

        // Premultiplied-alpha "over" blend: dst = src + dst * (1 - srcA).
        let dst = dstBytes.assumingMemoryBound(to: UInt8.self)
        let dstBytesPerRow = pixelWidth * 4
        let blendH = min(dstH, srcH)
        let blendW = min(dstW, srcW)

        for row in 0..<blendH {
            let dstRowBase = (dstY + row) * dstBytesPerRow + dstX * 4
            let srcRowBase = row * gooBytesPerRow
            for col in 0..<blendW {
                let si = srcRowBase + col * 4
                let di = dstRowBase + col * 4
                let srcA = Int(gooPixels[si + 3])
                guard srcA > 0 else { continue }
                let oneMinusSrcA = 255 - srcA
                // BGRA layout: [0]=B, [1]=G, [2]=R, [3]=A
                dst[di + 0] = UInt8(min(Int(gooPixels[si + 0]) + Int(dst[di + 0]) * oneMinusSrcA / 255, 255))
                dst[di + 1] = UInt8(min(Int(gooPixels[si + 1]) + Int(dst[di + 1]) * oneMinusSrcA / 255, 255))
                dst[di + 2] = UInt8(min(Int(gooPixels[si + 2]) + Int(dst[di + 2]) * oneMinusSrcA / 255, 255))
                dst[di + 3] = UInt8(min(srcA + Int(dst[di + 3]) * oneMinusSrcA / 255, 255))
            }
        }
    }

    private func ensureTexture(width: Int, height: Int) {
        guard (width, height) != captureSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead]
        desc.storageMode = .shared

        captureTexture = device.makeTexture(descriptor: desc)
        captureTexture?.label = "CrystalKit Backdrop"
        captureSize = (width, height)
    }
}

#elseif os(visionOS)
import UIKit

@MainActor
final class BedrockCapture {

    private let device: MTLDevice
    private var captureTexture: MTLTexture?
    private var captureSize: (Int, Int) = (0, 0)

    init(device: MTLDevice) {
        self.device = device
    }

    /// visionOS v1 fallback: returns a solid semi-transparent texture.
    ///
    /// Real backdrop capture on visionOS requires CompositorServices, which is
    /// a significantly different architecture. This fallback provides a tinted
    /// blur approximation.
    func capture(behind view: UIView) -> MTLTexture? {
        let scale: CGFloat = 2.0
        let pixelWidth = max(Int(view.bounds.width * scale), 1)
        let pixelHeight = max(Int(view.bounds.height * scale), 1)

        ensureTexture(width: pixelWidth, height: pixelHeight)
        guard let texture = captureTexture else { return nil }

        // Fill with a neutral gray (the shader will blur/tint it).
        let bytesPerRow = pixelWidth * 4
        let totalBytes = bytesPerRow * pixelHeight
        var pixels = [UInt8](repeating: 0, count: totalBytes)
        for i in stride(from: 0, to: totalBytes, by: 4) {
            pixels[i] = 180     // B
            pixels[i + 1] = 180 // G
            pixels[i + 2] = 180 // R
            pixels[i + 3] = 255 // A
        }
        pixels.withUnsafeBufferPointer { buf in
            texture.replace(
                region: MTLRegionMake2D(0, 0, pixelWidth, pixelHeight),
                mipmapLevel: 0,
                withBytes: buf.baseAddress!,
                bytesPerRow: bytesPerRow
            )
        }

        logger.info("visionOS backdrop capture: using fallback solid texture. Real backdrop requires CompositorServices.")

        return texture
    }

    private func ensureTexture(width: Int, height: Int) {
        guard (width, height) != captureSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead]
        desc.storageMode = .shared

        captureTexture = device.makeTexture(descriptor: desc)
        captureTexture?.label = "CrystalKit Backdrop (visionOS fallback)"
        captureSize = (width, height)
    }
}

#endif
