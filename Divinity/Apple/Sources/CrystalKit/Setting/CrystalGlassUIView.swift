// SettingUIView.swift
// CrystalKit
//
// iOS: UIView with CAMetalLayer that captures the backdrop behind itself,
// blurs it, and composites the Liquid Glass effect using Metal shaders.

#if os(iOS) || os(visionOS)
import UIKit
import Metal
import QuartzCore
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "SettingUIView")

@MainActor
final class SettingUIView: UIView {

    // MARK: - Configuration

    var style: FacetStyle = .regular { didSet { needsRender = true; backdropDirty = true } }
    var shape: ShapeDescriptor = .roundedRect() { didSet { needsRender = true; backdropDirty = true } }
    var backdropProvider: (any BedrockProvider)? { didSet { needsRender = true; backdropDirty = true } }

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

    /// One-shot flag for logging when goo texture is first received from shared store.
    private var _loggedGooTexture = false

    /// Externally-provided gaze/cursor position in screen coordinates.
    /// When set, overrides the built-in hover tracking for cursor-mode light source.
    /// Proximity check is done in `cursorWorldPosition()` every render frame.
    var externalGazePoint: CGPoint? {
        didSet { needsRender = true }
    }

    // MARK: - Cursor / Pointer Tracking

    /// Current pointer position in view-local coordinates (`nil` = not tracked / outside).
    private var cursorPosition: CGPoint?
    /// Previous frame's light direction/hover — used to skip render when cursor is stationary.
    private var lastTiltDirection: CGPoint?
    private var lastHoverPoint: CGPoint?

    /// Transparent overlay that captures hover events (since this view has interaction disabled).



    // MARK: - Backdrop Cache

    /// True when the content behind this view has changed and a fresh capture is needed.
    private var backdropDirty = true
    private var cachedBackdropTexture: MTLTexture?
    private var cachedRegionCopy: MTLTexture?
    private var cachedBlurredTexture: MTLTexture?

    /// Generation of the last provider content we blurred. When the provider's
    /// `contentGeneration` advances, we know new pixels are ready and re-blur.
    private var lastBlurredGeneration: UInt64 = 0

    /// Last goo render count seen — used to detect new goo frames without dirtying every frame.
    /// The goo view increments `ConfluenceOutputStore.renderCount` after each render pass.
    private var lastGooRenderCount: UInt64 = 0

    /// Snapshot of the goo texture and scope frame captured at the moment backdropDirty was set.
    /// Prevents a TOCTOU race where the store is cleared/reset between the dirty check and render.
    private var pendingGooTexture: MTLTexture?
    private var pendingGooFrame: CGRect = .zero

    // MARK: - Metal State

    private var device: MTLDevice!
    private var commandQueue: MTLCommandQueue!
    private var glassRenderer: Holodeck!
    private var blurRenderer: FrostHolodeck!
    private var backdropCapture: BedrockCapture!

    nonisolated(unsafe) private var displayLink: CADisplayLink?
    private var needsRender = true

    override class var layerClass: AnyClass { CAMetalLayer.self }
    private var metalLayer: CAMetalLayer { layer as! CAMetalLayer }

    // MARK: - Lifecycle

    override init(frame: CGRect) {
        super.init(frame: frame)
        setup()
    }

    required init?(coder: NSCoder) {
        super.init(coder: coder)
        setup()
    }

    private func setup() {
        guard let mtlDevice = MTLCreateSystemDefaultDevice() else {
            logger.error("No Metal device available")
            return
        }

        device = mtlDevice
        commandQueue = device.makeCommandQueue()

        metalLayer.device = device
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = false
        metalLayer.isOpaque = false
        metalLayer.backgroundColor = UIColor.clear.cgColor
        metalLayer.contentsScale = UIScreen.main.scale

        blurRenderer = FrostHolodeck(device: device)
        glassRenderer = Holodeck(device: device, blurRenderer: blurRenderer)
        backdropCapture = BedrockCapture(device: device)

        backgroundColor = .clear
        isUserInteractionEnabled = false

        startDisplayLink()

        #if !os(visionOS)
        GleamTiltTracker.shared.addConsumer()
        GleamHoverTracker.shared.addConsumer()
        #endif
    }

    deinit {
        displayLink?.invalidate()
        displayLink = nil
        #if !os(visionOS)
        // UIView deinit always runs on main thread; Swift 6 needs explicit annotation.
        MainActor.assumeIsolated {
            GleamTiltTracker.shared.removeConsumer()
            GleamHoverTracker.shared.removeConsumer()
        }
        #endif
    }

    // MARK: - Cursor / Pointer Tracking

    @objc private func handleHover(_ recognizer: UIHoverGestureRecognizer) {
        guard case .cursor = style.lightSource else { return }
        switch recognizer.state {
        case .began, .changed:
            cursorPosition = recognizer.location(in: self)
            needsRender = true
        case .ended, .cancelled:
            // Keep last cursor position — light stays pooled at nearest edge.
            break
        default:
            break
        }
    }

    /// Returns cursor world position for the shader, or `nil` if cursor tracking is inactive.
    /// For standalone views, world coords = element-local coords centered at origin.
    ///
    /// Priority: tilt direction (iPhone) > external light point > hover cursor (iPad).
    /// No proximity check here — the shader's `wideProximity` Gaussian handles
    /// distance-based falloff, giving smooth deactivation as the light moves away.
    private func cursorWorldPosition() -> SIMD2<Float>? {
        guard case .cursor = style.lightSource else { return nil }

        // Tilt tracker: normalized direction → map to this view's own bounds.
        // Every glass shape gets the light from the same direction.
        if let dir = GleamTiltTracker._sharedLightDirection {
            let wx = Float(dir.x) * Float(bounds.width)
            let wy = Float(dir.y) * Float(bounds.height)
            return SIMD2<Float>(wx, wy)
        }

        // External gaze/light point (screen coords → view-local)
        if let lightScreen = externalGazePoint, let window {
            let windowPoint = window.convert(lightScreen, from: nil)
            let local = convert(windowPoint, from: window)
            let wx = Float(local.x - bounds.width * 0.5)
            let wy = Float(local.y - bounds.height * 0.5)
            return SIMD2<Float>(wx, wy)
        }

        // iPad hover tracker (global window-level hover capture)
        if let hoverScreen = GleamHoverTracker._sharedHoverPoint, let window {
            let windowPoint = window.convert(hoverScreen, from: nil)
            let local = convert(windowPoint, from: window)
            let wx = Float(local.x - bounds.width * 0.5)
            let wy = Float(local.y - bounds.height * 0.5)
            return SIMD2<Float>(wx, wy)
        }

        // Fall back to per-view hover cursor tracking
        guard let cursor = cursorPosition else { return nil }
        let wx = Float(cursor.x - bounds.width * 0.5)
        let wy = Float(cursor.y - bounds.height * 0.5)
        return SIMD2<Float>(wx, wy)
    }

    // MARK: - Layout

    override func layoutSubviews() {
        super.layoutSubviews()
        let scale = window?.screen.scale ?? UIScreen.main.scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )
        needsRender = true
        backdropDirty = true
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        needsRender = true
        backdropDirty = true
    }

    override func didMoveToSuperview() {
        super.didMoveToSuperview()
    }

    // MARK: - Display Link

    private func startDisplayLink() {
        let link = CADisplayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    private func stopDisplayLink() {
        displayLink?.invalidate()
        displayLink = nil
    }

    @objc private func displayLinkFired() {
        // Poll shared light point every frame (bypasses SwiftUI coalescing).
        // On iPhone: tilt tracker drives the light.
        // On iPad: hover cursor (Apple Pencil / trackpad) drives it.
        // Only mark dirty when the tracked value actually changed.
        if case .cursor = style.lightSource {
            let currentTilt = GleamTiltTracker._sharedLightDirection
            let currentHover = GleamHoverTracker._sharedHoverPoint
            if currentTilt != lastTiltDirection || currentHover != lastHoverPoint {
                lastTiltDirection = currentTilt
                lastHoverPoint = currentHover
                needsRender = true
            }
        }
        // Dirty the backdrop whenever the goo view has rendered a new frame,
        // but only if this glass view's frame actually overlaps the goo scope.
        // Glass views outside the scope (or racing before goo publishes) skip
        // the expensive CALayer.render and stay idle.
        let currentGooCount = ConfluenceOutputStore.renderCount
        if currentGooCount != lastGooRenderCount {
            lastGooRenderCount = currentGooCount
            // Snapshot goo state now — the store may change by the time renderIfNeeded runs.
            let gooTex = ConfluenceOutputStore.texture
            let gooFrame = ConfluenceOutputStore.scopeFrame
            if let tex = gooTex,
               let window,
               !gooFrame.isNull, gooFrame.width > 0,
               convert(bounds, to: window).intersects(gooFrame) {
                pendingGooTexture = tex
                pendingGooFrame = gooFrame
                backdropDirty = true
                needsRender = true
            }
        }
        renderIfNeeded()
    }

    // MARK: - Render

    private func renderIfNeeded() {
        // Check if the backdrop provider has new content we haven't blurred yet.
        if let provider = backdropProvider,
           provider.contentGeneration != lastBlurredGeneration {
            backdropDirty = true
            needsRender = true
        }

        guard needsRender else { return }
        guard glassRenderer.isReady else { return }
        guard bounds.width > 0, bounds.height > 0 else { return }
        guard let window else { return }
        guard let drawable = metalLayer.nextDrawable() else { return }
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return }

        needsRender = false

        // 1. Capture backdrop only when dirty — skip the expensive CALayer.render
        //    on idle frames and reuse the last captured textures.
        if backdropDirty {
            backdropDirty = false

            // Use half-scale capture for high-frost glass — blur destroys fine detail
            // anyway, and quarters the pixel count on 3x devices. Clear/low-frost glass
            // always captures at full scale to preserve sharp refraction.
            let blurRadius = Holodeck.blurRadius(forFrost: style.frost)
            let captureScale: CGFloat = blurRadius > 32
                ? max(window.screen.scale * 0.5, 1.0)
                : window.screen.scale

            // Read goo state at capture time. All display link callbacks fire
            // sequentially on the main thread, so the store is stable within a tick.
            // Use pendingGooTexture if set (from renderCount path); otherwise read
            // the store directly (for layout/style-triggered captures).
            let gooTex = pendingGooTexture ?? ConfluenceOutputStore.texture
            let gooFrame = pendingGooTexture != nil ? pendingGooFrame : ConfluenceOutputStore.scopeFrame
            pendingGooTexture = nil
            pendingGooFrame = .zero

            // One-shot log to confirm the goo texture is wired up.
            if gooTex != nil, !_loggedGooTexture {
                _loggedGooTexture = true
                logger.info("Standalone glass reading goo from shared store: \(gooTex!.width)x\(gooTex!.height), scopeFrame=\(gooFrame.debugDescription)")
            }

            let freshBackdrop: MTLTexture?
            if let provider = backdropProvider {
                let viewFrameInWindow = convert(bounds, to: window)
                freshBackdrop = provider.backdropTexture(for: viewFrameInWindow, scale: captureScale)
                lastBlurredGeneration = provider.contentGeneration
            } else {
                freshBackdrop = backdropCapture.capture(
                    behind: self,
                    gooTexture: gooTex,
                    gooFrame: gooFrame,
                    captureScale: captureScale
                )
            }

            if let backdrop = freshBackdrop {
                cachedBackdropTexture = backdrop
                updateBackdropLuminance(from: backdrop)

                // 2. Blur the fresh capture and cache results.
                let fullRegion = MTLRegionMake2D(0, 0, backdrop.width, backdrop.height)
                if let regionCopy = blurRenderer.copyRegion(from: backdrop, region: fullRegion, commandBuffer: commandBuffer) {
                    cachedRegionCopy = regionCopy
                    if blurRadius > 0 {
                        cachedBlurredTexture = blurRenderer.blur(source: regionCopy, radius: blurRadius, commandBuffer: commandBuffer)
                    } else {
                        cachedBlurredTexture = regionCopy
                    }
                }
            }
        }

        // Require valid cached textures to composite.
        guard let regionCopy = cachedRegionCopy,
              let blurred = cachedBlurredTexture else { return }

        // 3. Build uniforms (pass cursor world position if tracking is active).
        let cursorWP = cursorWorldPosition() ?? SIMD2<Float>(-.infinity, -.infinity)

        // On iPhone, modulate light intensity by ambient screen brightness.
        var renderStyle = style
        if GleamTiltTracker._sharedLightDirection != nil {
            renderStyle.lightIntensity *= GleamTiltTracker._sharedBrightnessIntensity
        }

        let viewSize = CGSize(width: bounds.width, height: bounds.height)
        let texSize = SIMD2<Float>(Float(blurred.width), Float(blurred.height))
        let uniforms = Holodeck.buildUniforms(
            style: renderStyle,
            shape: shape,
            size: viewSize,
            blurredTextureSize: texSize,
            blurPadding: .zero,
            cursorWorldPos: cursorWP
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
            backingScale: Float(metalLayer.contentsScale),
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
            sharpBackground: regionCopy,
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

        // 6. Optional: render luminance mask for per-pixel adaptive foreground.
        if style.needsLuminanceMask {
            let scale = Float(metalLayer.contentsScale)
            let maskTex = glassRenderer.encodeLuminanceMask(
                backgroundTexture: regionCopy,
                viewSize: viewSize,
                backingScale: scale,
                commandBuffer: commandBuffer
            )
            // iOS uses .shared storage — no blit synchronization needed.
            nonisolated(unsafe) let sendableTex = maskTex
            commandBuffer.addCompletedHandler { [weak self] _ in
                DispatchQueue.main.async {
                    self?.onLuminanceMaskUpdate?(sendableTex)
                }
            }
        } else {
            onLuminanceMaskUpdate?(nil)
        }

        commandBuffer.present(drawable)
        commandBuffer.commit()
    }

    // MARK: - Backdrop Luminance

    /// Samples 5 pixels from the CPU-readable backdrop texture and publishes
    /// the average luminance when it changes meaningfully (delta > 0.05).
    /// Also extracts the averaged RGB color.
    private func updateBackdropLuminance(from texture: MTLTexture) {
        guard onLuminanceUpdate != nil || onBackdropColorUpdate != nil else { return }

        let tw = texture.width
        let th = texture.height
        guard tw > 0, th > 0 else { return }

        let cx = tw / 2
        let cy = th / 2

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
        let spanX = max(tw * 3 / 10, 1)
        let spanY = max(th * 3 / 10, 1)
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
