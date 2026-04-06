// ImageBedrockProvider.swift
// CrystalKit
//
// A ready-made BedrockProvider for SwiftUI apps that renders a
// CGImage (from ImageRenderer, NSImage, or any other source) as the glass
// backdrop. Holds a full-content Metal texture and crops view-sized slices
// on demand — no CGWindowListCreateImage, no feedback loop, no permissions.
//
// Uses triple-buffered textures to prevent GPU race conditions: the glass
// renderer reads from the current texture while the provider writes to
// the next slot. The third buffer absorbs in-flight GPU reads that haven't
// completed yet (SettingNSView commits with waitUntilScheduled, not
// waitUntilCompleted, so reads may still be executing when the provider
// cycles back around).

#if os(macOS)
import CoreGraphics
import Metal
import QuartzCore
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "ImageBackdropProvider")

/// A concrete `BedrockProvider` backed by a `CGImage`.
///
/// Use this when your app's backdrop is a SwiftUI view (image, gradient, canvas)
/// that you can render to a `CGImage` via `ImageRenderer` or obtain from an
/// `NSImage`. The provider holds the full-content texture and crops view-sized
/// slices on each frame, giving glass views a ghost-free backdrop.
///
/// ## Usage
///
/// ```swift
/// @State private var provider = ImageBedrockProvider()
///
/// var body: some View {
///     ZStack {
///         MyBackdropView()
///         GlassContent()
///     }
///     .bedrock(provider)
///     .onGeometryChange(for: CGSize.self, of: \.size) { size in
///         let renderer = ImageRenderer(content:
///             MyBackdropView().frame(width: size.width, height: size.height))
///         renderer.scale = NSScreen.main?.backingScaleFactor ?? 2
///         if let cg = renderer.cgImage { provider.update(with: cg) }
///     }
/// }
/// ```
@MainActor
public final class ImageBedrockProvider: BedrockProvider {

    // MARK: - Metal State

    private let device: MTLDevice
    private let commandQueue: MTLCommandQueue

    /// Triple-buffered full-content textures. The glass renderer reads from
    /// `fullTextures[currentIndex]` while the provider writes to the next slot.
    /// Three buffers (not two) because SettingNSView commits with
    /// `waitUntilScheduled()` — the GPU may still be reading a texture when
    /// the provider cycles back to overwrite it. The third buffer absorbs
    /// this in-flight read latency.
    private var fullTextures: [MTLTexture?] = [nil, nil, nil]
    private static let bufferCount = 3

    /// Index of the texture the glass renderer should read (most recently written).
    private var currentIndex: Int = 0

    /// Reusable crop texture returned by `backdropTexture(for:scale:)`.
    private var cropTexture: MTLTexture?
    private var cropTextureSize: (Int, Int) = (0, 0)

    // MARK: - Provider State

    public private(set) var contentGeneration: UInt64 = 0

    /// The content area size in points that the texture represents.
    /// Used to derive the effective pixel scale (`texture.width / pointSize.width`)
    /// instead of trusting the NSView's backing scale factor.
    private var contentPointSize: CGSize = .zero

    /// The origin of the texture's content area in NSView window coordinates
    /// (points, bottom-left origin). Used to offset crop calculations when the
    /// texture doesn't cover the full window — e.g. when `ImageRenderer` only
    /// renders a detail pane that starts after a sidebar.
    public var contentOrigin: CGPoint = .zero

    // MARK: - Init

    /// Creates a new image backdrop provider.
    ///
    /// - Parameter device: The Metal device to use. Defaults to the system default.
    ///   Returns `nil` if no Metal device is available.
    public init?(device: MTLDevice? = nil) {
        guard let dev = device ?? MTLCreateSystemDefaultDevice() else {
            logger.error("No Metal device available for ImageBedrockProvider")
            return nil
        }
        guard let queue = dev.makeCommandQueue() else {
            logger.error("Failed to create command queue for ImageBedrockProvider")
            return nil
        }
        self.device = dev
        self.commandQueue = queue
    }

    // MARK: - Update

    /// Uploads a new full-content backdrop image.
    ///
    /// Call this whenever the backdrop content changes (mode switch, image drop,
    /// gradient change, window resize). The image should represent the entire
    /// content area at pixel resolution (i.e. already scaled by the backing
    /// scale factor).
    ///
    /// - Parameters:
    ///   - image: The full-content `CGImage` (e.g. from `ImageRenderer.cgImage`).
    ///   - pointSize: The content area size in points. Used to compute the effective
    ///     pixel-to-point scale for crop math (instead of trusting the NSView's
    ///     backing scale factor, which may not match `ImageRenderer`'s actual output).
    public func update(with image: CGImage, pointSize: CGSize) {
        let w = image.width
        let h = image.height
        guard w > 0, h > 0 else { return }

        let writeIndex = (currentIndex + 1) % Self.bufferCount
        ensureTexture(at: writeIndex, width: w, height: h, pixelFormat: .bgra8Unorm)
        guard let tex = fullTextures[writeIndex] else { return }

        // Draw into a BGRA context matching Metal's expected pixel format.
        let bytesPerRow = w * 4
        guard let ctx = CGContext(
            data: nil,
            width: w,
            height: h,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
        ) else {
            logger.error("Failed to create CGContext for backdrop upload")
            return
        }

        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))

        guard let data = ctx.data else { return }
        tex.replace(
            region: MTLRegionMake2D(0, 0, w, h),
            mipmapLevel: 0,
            withBytes: data,
            bytesPerRow: bytesPerRow
        )

        // Flush the CPU write to the GPU copy of the managed texture.
        // Without this, the glass shader may read stale pixels.
        syncManagedTexture(tex)

        currentIndex = writeIndex
        contentPointSize = pointSize
        contentGeneration += 1

        logger.info("Backdrop texture: \(w)×\(h) px for \(Int(pointSize.width))×\(Int(pointSize.height)) pt → scale \(String(format: "%.2f", Double(w) / pointSize.width))")
    }

    /// Updates the backdrop by GPU-blitting from an existing Metal texture.
    ///
    /// This bypasses the CGImage round-trip entirely — a single GPU blit (~0.1ms)
    /// vs. CPU readback + CGImage + upload (~10–15ms). Use when the source is
    /// already a Metal texture (e.g. a canvas snapshot that was blit from a drawable).
    ///
    /// - Parameters:
    ///   - texture: Source texture on the same Metal device. Must already be
    ///     GPU-synchronized (e.g. via `blit.synchronize` + `waitUntilCompleted`).
    ///   - pointSize: The content area size in points.
    public func update(withTexture texture: MTLTexture, pointSize: CGSize) {
        let w = texture.width
        let h = texture.height
        guard w > 0, h > 0 else { return }

        // Write to the next buffer — not the one glass is currently reading,
        // and not the one a previous reader may still have in flight on the GPU.
        let writeIndex = (currentIndex + 1) % Self.bufferCount
        ensureTexture(at: writeIndex, width: w, height: h, pixelFormat: texture.pixelFormat)

        guard let target = fullTextures[writeIndex],
              let cmdBuf = commandQueue.makeCommandBuffer(),
              let blit = cmdBuf.makeBlitCommandEncoder() else { return }

        blit.copy(
            from: texture, sourceSlice: 0, sourceLevel: 0,
            sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
            sourceSize: MTLSize(width: w, height: h, depth: 1),
            to: target, destinationSlice: 0, destinationLevel: 0,
            destinationOrigin: MTLOrigin(x: 0, y: 0, z: 0)
        )
        blit.endEncoding()
        cmdBuf.commit()
        cmdBuf.waitUntilCompleted()

        // Swap: glass now reads the freshly written texture.
        currentIndex = writeIndex
        contentPointSize = pointSize
        contentGeneration += 1
    }

    // MARK: - BedrockProvider

    public func backdropTexture(for rect: CGRect, scale: CGFloat) -> MTLTexture? {
        // Return the current front buffer — SettingNSView handles cropping in its
        // own render pass, which avoids integer-pixel jitter from GPU blit crops.
        return fullTextures[currentIndex]
    }

    /// The effective pixel scale of the full texture (texture pixels / content points).
    /// Used by SettingNSView to convert point-based crop rects to pixel coordinates.
    public var effectiveScale: CGFloat {
        guard let full = fullTextures[currentIndex], contentPointSize.width > 0 else { return 1.0 }
        return CGFloat(full.width) / contentPointSize.width
    }

    // MARK: - Texture Management

    /// Ensures the texture at the given buffer index matches the required dimensions
    /// and pixel format. Recreates it if mismatched.
    private func ensureTexture(at index: Int, width: Int, height: Int, pixelFormat: MTLPixelFormat) {
        if let existing = fullTextures[index],
           existing.width == width, existing.height == height,
           existing.pixelFormat == pixelFormat {
            return
        }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: pixelFormat,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead]
        desc.storageMode = .managed

        fullTextures[index] = device.makeTexture(descriptor: desc)
        let slotName = ["A", "B", "C"][index]
        fullTextures[index]?.label = "CrystalKit ImageBackdrop (\(slotName))"
    }

    /// Flushes a managed texture's CPU buffer to the GPU.
    /// Required after `replace()` — without it, shaders on other command
    /// queues may read stale pixels.
    private func syncManagedTexture(_ texture: MTLTexture) {
        guard let cmdBuf = commandQueue.makeCommandBuffer(),
              let blit = cmdBuf.makeBlitCommandEncoder() else { return }
        blit.synchronize(resource: texture)
        blit.endEncoding()
        cmdBuf.commit()
        cmdBuf.waitUntilCompleted()
    }

    private func ensureCropTexture(width: Int, height: Int) {
        guard (width, height) != cropTextureSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead]
        desc.storageMode = .managed

        cropTexture = device.makeTexture(descriptor: desc)
        cropTexture?.label = "CrystalKit ImageBackdrop (crop)"
        cropTextureSize = (width, height)
    }
}

#endif
