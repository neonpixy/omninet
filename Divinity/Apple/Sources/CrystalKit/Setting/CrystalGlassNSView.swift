// SettingNSView.swift
// CrystalKit
//
// macOS: NSView with CAMetalLayer that captures the backdrop behind itself,
// blurs it, and composites the Liquid Glass effect using Metal shaders.
// Uses NSView.displayLink(target:selector:) for frame timing (macOS 14+).

#if os(macOS)
import AppKit
import Metal
import QuartzCore
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "SettingNSView")

@MainActor
final class SettingNSView: NSView {

    // MARK: - Configuration

    var style: FacetStyle = .regular { didSet { needsRender = true; blurDirty = true } }
    var shape: ShapeDescriptor = .roundedRect() { didSet { needsRender = true } }
    var backdropProvider: (any BedrockProvider)? { didSet { needsRender = true; backdropDirty = true } }

    /// External offset from SwiftUI `.facetOffset()`.
    /// `.offset()` applies a CALayer transform that `convert(bounds, to: nil)` doesn't see.
    /// This compensates so UV coordinates follow the visual position.
    var externalOffset: CGSize = .zero { didSet { needsRender = true } }

    /// Called when the average background luminance changes meaningfully.
    /// Value is 0 (black) to 1 (white). Used by the modifier to set foreground style.
    var onLuminanceUpdate: ((CGFloat) -> Void)?
    /// Called when the average background color changes meaningfully (sRGB 0-1).
    var onBackdropColorUpdate: ((SIMD3<Float>) -> Void)?
    /// Called with the per-pixel luminance mask texture after each render when
    /// `style.needsLuminanceMask` is true. `nil` when disabled.
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    private var lastReportedLuminance: CGFloat = 0.5
    private var lastReportedColor: SIMD3<Float> = SIMD3(repeating: 0.5)

    /// Externally-provided gaze/cursor position in screen coordinates.
    /// When set, overrides the built-in mouse tracking for cursor-mode light source.
    var externalGazePoint: CGPoint? {
        didSet {
            if let point = externalGazePoint, case .cursor = style.lightSource {
                // Convert screen coords to view-local coords
                if let window {
                    let windowPoint = window.convertPoint(fromScreen: point)
                    cursorPosition = convert(windowPoint, from: nil)
                } else {
                    cursorPosition = point
                }
                needsRender = true
            }
        }
    }

    // MARK: - Cursor Tracking

    /// Current cursor position in view-local coordinates (`nil` = not tracked / outside window).
    private var cursorPosition: CGPoint?
    nonisolated(unsafe) private var mouseMonitor: Any?

    // MARK: - Backdrop Cache

    /// True when the content behind this view has changed and a fresh capture is needed.
    /// Set by notification observers and layout changes; cleared after each capture.
    private var backdropDirty = true

    /// True when the cached blur needs recomputing (e.g. frost changed, or fresh capture).
    /// Separate from `backdropDirty` because style changes need a re-blur but NOT a recapture.
    private var blurDirty = true

    /// Tracks the view's last known position in the window so we can detect movement
    /// (e.g. when the glass panel is dragged within the same window).
    private var lastFrameInWindow: CGRect = .zero


    private var cachedBackdropTexture: MTLTexture?
    private var cachedRegionCopy: MTLTexture?
    private var cachedBlurredTexture: MTLTexture?
    /// Generation of the last provider content we blurred. When the provider's
    /// `contentGeneration` advances, we know new pixels are ready and re-blur.
    private var lastBlurredGeneration: UInt64 = 0

    // MARK: - Metal State

    private var metalLayer: CAMetalLayer!
    private var device: MTLDevice!
    private var commandQueue: MTLCommandQueue!
    private var glassRenderer: Holodeck!
    private var blurRenderer: FrostHolodeck!
    private var backdropCapture: BedrockCapture!

    nonisolated(unsafe) private var frameLink: CADisplayLink?
    private var needsRender = true
    private var isSetup = false


    // MARK: - Lifecycle

    override init(frame: NSRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        wantsLayer = true

        guard let mtlDevice = MTLCreateSystemDefaultDevice() else {
            logger.error("No Metal device available")
            return
        }

        device = mtlDevice
        commandQueue = device.makeCommandQueue()

        let layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = false
        layer.isOpaque = false
        layer.backgroundColor = .clear
        layer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        // Synchronize drawable presentation with Core Animation transactions.
        // Without this, the Metal content appears 1 vsync after the view moves,
        // causing visible jitter against the SwiftUI backdrop during drag/scroll.
        layer.presentsWithTransaction = true
        self.layer = layer
        metalLayer = layer

        blurRenderer = FrostHolodeck(device: device)
        glassRenderer = Holodeck(device: device, blurRenderer: blurRenderer)
        backdropCapture = BedrockCapture(device: device)

        isSetup = true
        setupCursorTracking()
    }

    deinit {
        frameLink?.invalidate()
        frameLink = nil
        if let monitor = mouseMonitor {
            NSEvent.removeMonitor(monitor)
        }
        NotificationCenter.default.removeObserver(self)
        // Note: unregisterFromWindowRegistry() not called here because deinit
        // isn't @MainActor-isolated — but NSHashTable<weak> handles it
        // automatically when this object deallocates.
    }

    // MARK: - Cursor Tracking

    private func setupCursorTracking() {
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged]) { [weak self] event in
            self?.handleMouseEvent(event)
            return event
        }
    }

    private func handleMouseEvent(_ event: NSEvent) {
        guard case .cursor = style.lightSource else { return }
        guard let window, window.isVisible else { return }
        let localPoint = convert(event.locationInWindow, from: nil)
        if let prev = cursorPosition {
            let dx = localPoint.x - prev.x
            let dy = localPoint.y - prev.y
            guard dx * dx + dy * dy > 0.25 else { return }  // skip sub-pixel moves
        }
        cursorPosition = localPoint
        needsRender = true
    }

    /// Returns cursor world position for the shader, or `nil` if cursor tracking is inactive.
    /// For standalone views, world coords = element-local coords centered at origin.
    private func cursorWorldPosition() -> SIMD2<Float>? {
        guard case .cursor = style.lightSource,
              let cursor = cursorPosition else { return nil }

        // NSView: y increases upward; shader world coords are y-down with (0,0) at element center.
        let wx = Float(cursor.x - bounds.width * 0.5)
        let wy = Float((bounds.height - cursor.y) - bounds.height * 0.5)
        return SIMD2<Float>(wx, wy)
    }

    // MARK: - Layout

    override func layout() {
        super.layout()
        guard let metalLayer else { return }
        let scale = window?.backingScaleFactor ?? 2.0
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )
        needsRender = true
        backdropDirty = true
    }

    override func setFrameOrigin(_ newOrigin: NSPoint) {
        super.setFrameOrigin(newOrigin)
        needsRender = true
        // Render immediately at the exact moment AppKit repositions the view,
        // rather than waiting for the next display link tick (which may read a
        // stale frame position). This eliminates the timing gap between the
        // view's physical position and the Metal content's UV coordinates.
        renderIfNeeded()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            metalLayer?.contentsScale = window?.backingScaleFactor ?? 2.0
            needsRender = true
            backdropDirty = true
            if frameLink == nil, isSetup {
                startDisplayLink()
            }
            registerBackdropObservers()
        } else {
            frameLink?.invalidate()
            frameLink = nil
            unregisterBackdropObservers()
        }
    }

    // MARK: - Backdrop Invalidation

    /// Notification name used by Swiftlight's canvas render loop. CrystalKit observes this
    /// string directly — no import of Swiftlight required. In other apps, window move/resize
    /// notifications (below) cover the common cases; apps can also call `invalidateBackdrop()`
    /// explicitly for finer-grained control.
    private static let canvasNeedsRenderName = Notification.Name("metalCanvasNeedsRender")

    private var backdropObserversRegistered = false

    private func registerBackdropObservers() {
        guard !backdropObserversRegistered else { return }
        backdropObserversRegistered = true
        let nc = NotificationCenter.default
        nc.addObserver(self, selector: #selector(markBackdropDirty),
                       name: NSWindow.didMoveNotification, object: window)
        nc.addObserver(self, selector: #selector(markBackdropDirty),
                       name: NSWindow.didResizeNotification, object: window)
        nc.addObserver(self, selector: #selector(markBackdropDirty),
                       name: Self.canvasNeedsRenderName, object: nil)
    }

    private func unregisterBackdropObservers() {
        guard backdropObserversRegistered else { return }
        backdropObserversRegistered = false
        let nc = NotificationCenter.default
        nc.removeObserver(self, name: NSWindow.didMoveNotification, object: window)
        nc.removeObserver(self, name: NSWindow.didResizeNotification, object: window)
        nc.removeObserver(self, name: Self.canvasNeedsRenderName, object: nil)
    }

    @objc private func markBackdropDirty() {
        backdropDirty = true
        needsRender = true
    }

    /// Public escape hatch for apps that want explicit control over backdrop invalidation.
    /// Call this whenever the content behind the glass view has changed in a way that
    /// CrystalKit's built-in observers won't catch (e.g. custom render loops, non-window content).
    public func invalidateBackdrop() {
        backdropDirty = true
        needsRender = true
    }

    // MARK: - Display Link

    private func startDisplayLink() {
        // NSView.displayLink(target:selector:) — available macOS 14+.
        // Returns a CADisplayLink tied to this view's display. Must be added to a run loop.
        let link = (self as NSView).displayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        frameLink = link
    }

    private var maskFrameCounter: UInt64 = 0

    @objc private func displayLinkFired() {
        // Force backdrop refresh for dynamic content (video, animation behind glass).
        if style.dynamicBackdrop {
            backdropDirty = true
            needsRender = true
        }
        renderIfNeeded()
    }

    // MARK: - Render

    private func renderIfNeeded() {
        guard isSetup else { return }
        guard glassRenderer.isReady else { return }
        guard bounds.width > 0, bounds.height > 0 else { return }
        guard let window, window.isVisible else { return }

        // Track view movement within the window (drag, scroll).
        // With the full-texture approach, position changes only update the UV rect
        // in the shader — the blur and backdrop texture are position-independent.
        // For the CGWindowList fallback, recapture is needed since it provides the
        // full window which doesn't change with position either.
        //
        // externalOffset compensates for SwiftUI .offset() transforms that
        // convert(bounds, to:) can't see (they apply as CALayer transforms above
        // the NSView's hosting view). Y is negated because SwiftUI Y points down
        // while AppKit window coordinates point up.
        let rawFrame = convert(bounds, to: nil)
        let currentFrame = rawFrame.offsetBy(
            dx: externalOffset.width,
            dy: -externalOffset.height
        )
        let frameMoved = currentFrame != lastFrameInWindow
        if frameMoved {
            lastFrameInWindow = currentFrame
            needsRender = true
            // No blurDirty or backdropDirty — the full texture + UV rect handles it.
            // But we DO need to re-sample luminance/color at the new position.
            if let backdrop = cachedRegionCopy {
                let uv = computeUVRect(frame: currentFrame, texture: backdrop)
                updateBackdropLuminance(from: backdrop, uvOffset: uv.offset, uvScale: uv.scale)
            }
        }

        // Check if the backdrop provider has new content we haven't blurred yet.
        if let provider = backdropProvider,
           provider.contentGeneration != lastBlurredGeneration {
            backdropDirty = true
            needsRender = true
        }

        guard needsRender else { return }

        // 1. Capture the full window backdrop — only when dirty.
        //    On idle frames, reuse the cached full-window texture. Drag/scroll
        //    just shifts the crop region within this texture (no recapture).
        if backdropDirty {
            backdropDirty = false

            if let provider = backdropProvider {
                // Providers return the full backdrop texture — we crop in step 2,
                // using the same path as the fallback. This avoids integer-pixel
                // jitter that a separate GPU blit crop would introduce.
                if let tex = provider.backdropTexture(for: .zero, scale: 1) {
                    lastBlurredGeneration = provider.contentGeneration
                    cachedBackdropTexture = tex
                    blurDirty = true
                }
            } else {
                // Fallback: capture the full window via CGWindowListCreateImage.
                // This may include the glass effect from the previous frame (the
                // WindowServer compositor is async), producing a faint "ghost".
                // For ghost-free results, use a BedrockProvider instead.
                if let fullWindow = backdropCapture.captureFullWindow(for: self) {
                    cachedBackdropTexture = fullWindow
                    blurDirty = true
                }
            }
        }

        guard let drawable = metalLayer.nextDrawable() else { return }
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return }

        needsRender = false

        // 2. Blur the full backdrop texture (no blit crop — eliminates jitter).
        //    The shader uses floating-point UV offset/scale to sample the glass
        //    region, so there are no integer pixel boundaries to snap to.
        if blurDirty, let backdrop = cachedBackdropTexture {
            blurDirty = false

            cachedRegionCopy = backdrop  // Full texture, no crop
            // Sample at the glass view's current position in the full texture.
            let uv = computeUVRect(frame: currentFrame, texture: backdrop)
            updateBackdropLuminance(from: backdrop, uvOffset: uv.offset, uvScale: uv.scale)

            let blurRadius = Holodeck.blurRadius(forFrost: style.frost)
            if blurRadius > 0 {
                cachedBlurredTexture = blurRenderer.blur(source: backdrop, radius: blurRadius, commandBuffer: commandBuffer)
            } else {
                cachedBlurredTexture = backdrop
            }
        }

        // Require valid cached textures to composite.
        guard let fullBackdrop = cachedRegionCopy,
              let blurred = cachedBlurredTexture else { return }

        // 3. Compute floating-point UV rect for the glass region within the
        //    full backdrop texture. All coordinates are floats — no integers,
        //    no rounding, no jitter. Apply externalOffset (same as currentFrame above).
        let frame = convert(bounds, to: nil).offsetBy(
            dx: externalOffset.width,
            dy: -externalOffset.height
        )

        let scale: CGFloat
        let offsetX: CGFloat
        let offsetY: CGFloat

        if let imageProvider = backdropProvider as? ImageBedrockProvider {
            scale = imageProvider.effectiveScale
            offsetX = frame.origin.x - imageProvider.contentOrigin.x
            offsetY = frame.origin.y - imageProvider.contentOrigin.y
        } else {
            scale = backdropCapture.captureScale
            offsetX = frame.origin.x
            offsetY = frame.origin.y
        }

        let texW = CGFloat(fullBackdrop.width)
        let texH = CGFloat(fullBackdrop.height)

        // UV rect: where the glass sits within the full texture (all floats).
        let uvX = Float((offsetX * scale) / texW)
        let uvY = Float((texH - (offsetY + frame.height) * scale) / texH)
        let uvW = Float((bounds.width * scale) / texW)
        let uvH = Float((bounds.height * scale) / texH)

        let cropUVOffset = SIMD2<Float>(uvX, uvY)
        let cropUVScale = SIMD2<Float>(uvW, uvH)

        let cursorWP = cursorWorldPosition() ?? SIMD2<Float>(-.infinity, -.infinity)

        let viewSize = CGSize(width: bounds.width, height: bounds.height)
        let texSize = SIMD2<Float>(Float(blurred.width), Float(blurred.height))
        let uniforms = Holodeck.buildUniforms(
            style: style,
            shape: shape,
            size: viewSize,
            blurredTextureSize: texSize,
            blurPadding: .zero,
            cursorWorldPos: cursorWP,
            cropUVOffset: cropUVOffset,
            cropUVScale: cropUVScale
        )

        // 4. Render rim light to intermediate texture.
        let w = Float(viewSize.width)
        let h = Float(viewSize.height)
        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / w, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / h, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))
        let viewportUniforms = HolodeckViewportUniforms(viewProjection: ortho)

        guard let rimTex = glassRenderer.encodeRimLight(
            maskTexture: shape.sdfTexture,
            uniforms: uniforms,
            viewportUniforms: viewportUniforms,
            nodePosition: .zero,
            nodeSize: SIMD2<Float>(w, h),
            nodeRotation: 0,
            nodeOpacity: 1,
            nodeScale: 1,
            captureHalfExtent: SIMD2<Float>(w * 0.5, h * 0.5),
            backingScale: Float(metalLayer?.contentsScale ?? 2.0),
            commandBuffer: commandBuffer
        ) else { return }

        // 5. Render the glass composite into the drawable.
        let renderDesc = MTLRenderPassDescriptor()
        renderDesc.colorAttachments[0].texture = drawable.texture
        renderDesc.colorAttachments[0].loadAction = .clear
        renderDesc.colorAttachments[0].storeAction = .store
        renderDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderDesc) else { return }

        glassRenderer.encodeComposite(
            blurredBackground: blurred,
            sharpBackground: fullBackdrop,
            maskTexture: shape.sdfTexture,
            rimLightTexture: rimTex,
            uniforms: uniforms,
            viewportUniforms: viewportUniforms,
            nodePosition: .zero,
            nodeSize: SIMD2<Float>(w, h),
            nodeRotation: 0,
            nodeOpacity: 1,
            nodeScale: 1,
            captureHalfExtent: SIMD2<Float>(w * 0.5, h * 0.5),
            encoder: encoder
        )

        encoder.endEncoding()

        // 6. Optional: render luminance mask for zone-based adaptive foreground.
        //    Pass the UV rect so the mask covers only this glass panel's backdrop
        //    region — not the entire canvas. Zone AF children probe at UVs relative
        //    to the glass frame, so the mask must match that coordinate space.
        maskFrameCounter &+= 1
        if style.needsLuminanceMask {
            let maskScale = Float(metalLayer?.contentsScale ?? 2.0)
            let backdropUVRect = SIMD4<Float>(uvX, uvY, uvW, uvH)
            let maskTex = glassRenderer.encodeLuminanceMask(
                backgroundTexture: fullBackdrop,
                viewSize: viewSize,
                backingScale: maskScale,
                backdropUVRect: backdropUVRect,
                frost: Float(style.frost),
                tintColor: premultipliedRGBA(style.tintColor),
                tintOpacity: Float(style.tintOpacity),
                commandBuffer: commandBuffer
            )
            // Synchronize managed texture so CPU can read it for the SwiftUI mask view.
            if let maskTex, maskTex.storageMode == .managed {
                if let blit = commandBuffer.makeBlitCommandEncoder() {
                    blit.synchronize(resource: maskTex)
                    blit.endEncoding()
                }
            }
            // MTLTexture is not Sendable but is safe here — the texture outlives
            // the handler and is only read on main thread.
            nonisolated(unsafe) let sendableTex = maskTex
            commandBuffer.addCompletedHandler { [weak self] _ in
                DispatchQueue.main.async {
                    self?.onLuminanceMaskUpdate?(sendableTex)
                }
            }
        } else {
            onLuminanceMaskUpdate?(nil)
        }

        // With presentsWithTransaction, we commit first then present the drawable
        // synchronously within the current Core Animation transaction. This ensures
        // the Metal content and the view position appear on screen in the same frame.
        commandBuffer.commit()
        commandBuffer.waitUntilScheduled()
        drawable.present()

    }

    // MARK: - Backdrop Luminance

    /// Returns UV offset and scale for the glass view's position in the given texture.
    private func computeUVRect(frame: CGRect, texture: MTLTexture) -> (offset: SIMD2<Float>, scale: SIMD2<Float>) {
        let scale: CGFloat
        let offsetX: CGFloat
        let offsetY: CGFloat
        if let imageProvider = backdropProvider as? ImageBedrockProvider {
            scale = imageProvider.effectiveScale
            offsetX = frame.origin.x - imageProvider.contentOrigin.x
            offsetY = frame.origin.y - imageProvider.contentOrigin.y
        } else {
            scale = backdropCapture.captureScale
            offsetX = frame.origin.x
            offsetY = frame.origin.y
        }
        let texW = CGFloat(texture.width)
        let texH = CGFloat(texture.height)
        let uvX = Float((offsetX * scale) / texW)
        let uvY = Float((texH - (offsetY + frame.height) * scale) / texH)
        let uvW = Float((bounds.width * scale) / texW)
        let uvH = Float((bounds.height * scale) / texH)
        return (SIMD2(uvX, uvY), SIMD2(uvW, uvH))
    }

    /// Samples 5 pixels from the CPU-readable backdrop texture and publishes
    /// the average luminance when it changes meaningfully (delta > 0.05).
    /// Probes are centered on the glass view's UV position in the full texture.
    /// Also extracts the averaged RGB color.
    private func updateBackdropLuminance(from texture: MTLTexture, uvOffset: SIMD2<Float>, uvScale: SIMD2<Float>) {
        guard onLuminanceUpdate != nil || onBackdropColorUpdate != nil else { return }

        let tw = texture.width
        let th = texture.height
        guard tw > 0, th > 0 else { return }

        // Center probes on the glass view's position in the full texture.
        let cx = max(0, min(tw - 1, Int((uvOffset.x + uvScale.x * 0.5) * Float(tw))))
        let cy = max(0, min(th - 1, Int((uvOffset.y + uvScale.y * 0.5) * Float(th))))

        var pixel = [UInt8](repeating: 0, count: 4)
        let bytesPerRow = tw * 4

        // Center probe for color — single point preserves backdrop saturation.
        texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                         from: MTLRegionMake2D(cx, cy, 1, 1), mipmapLevel: 0)
        let centerColor = SIMD3<Float>(
            Float(pixel[2]) / 255.0,
            Float(pixel[1]) / 255.0,
            Float(pixel[0]) / 255.0
        )

        // 5-probe average for luminance — spatial spread is fine for brightness.
        let spanX = max(Int(uvScale.x * Float(tw) * 0.3), 1)
        let spanY = max(Int(uvScale.y * Float(th) * 0.3), 1)
        let probes = [
            (cx, cy),
            (min(cx + spanX, tw - 1), cy),
            (max(cx - spanX, 0), cy),
            (cx, min(cy + spanY, th - 1)),
            (cx, max(cy - spanY, 0))
        ]

        var totalLum: Float = 0
        for (px, py) in probes {
            texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                             from: MTLRegionMake2D(px, py, 1, 1), mipmapLevel: 0)
            let r = Float(pixel[2]) / 255.0
            let g = Float(pixel[1]) / 255.0
            let b = Float(pixel[0]) / 255.0
            totalLum += 0.2126 * r + 0.7152 * g + 0.0722 * b
        }

        let avgLum = CGFloat(totalLum / 5.0)
        if abs(avgLum - lastReportedLuminance) > 0.05 {
            lastReportedLuminance = avgLum
            onLuminanceUpdate?(avgLum)
        }

        let colorDelta = simd_length(centerColor - lastReportedColor)
        if colorDelta > 0.05 {
            lastReportedColor = centerColor
            onBackdropColorUpdate?(centerColor)
        }
    }
}

#endif
