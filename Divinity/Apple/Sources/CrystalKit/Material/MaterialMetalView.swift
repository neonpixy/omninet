// MaterialMetalView.swift
// CrystalKit
//
// Shared macOS NSView that hosts a CAMetalLayer and drives any MaterialRenderer.
// Handles Metal device setup, display link, layout, and the render loop.
// Material-specific logic lives in the renderer — this view is generic.

#if os(macOS)
import AppKit
import Metal
import QuartzCore
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "MaterialMetalView")

@MainActor
public final class MaterialMetalView: NSView {

    // MARK: - Configuration

    /// The renderer that produces the material effect. Set by the representable.
    public var renderer: (any MaterialRenderer)? { didSet { needsRender = true } }

    /// Shape to clip the material to.
    public var shape: ShapeDescriptor = .roundedRect() { didSet { needsRender = true } }

    /// When true, the display link fires every frame (for animated materials).
    /// When false, only renders when `needsRender` is set explicitly.
    public var isAnimating: Bool = false {
        didSet {
            if isAnimating { needsRender = true }
        }
    }

    /// Called every frame before encoding. Materials use this to update time-varying uniforms.
    /// The closure receives elapsed seconds since first render.
    public var onBeforeRender: ((Float) -> Void)?

    // MARK: - Metal State

    private(set) var device: MTLDevice!
    private var commandQueue: MTLCommandQueue!
    private var metalLayer: CAMetalLayer!
    nonisolated(unsafe) private var frameLink: CADisplayLink?
    private(set) var needsRender = true
    private var isSetup = false
    private var animationStartTime: CFTimeInterval?

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
        layer.presentsWithTransaction = true
        self.layer = layer
        metalLayer = layer

        isSetup = true
    }

    deinit {
        frameLink?.invalidate()
        frameLink = nil
    }

    // MARK: - Layout

    override public func layout() {
        super.layout()
        guard let metalLayer else { return }
        let scale = window?.backingScaleFactor ?? 2.0
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )
        needsRender = true
    }

    override public func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            metalLayer?.contentsScale = window?.backingScaleFactor ?? 2.0
            needsRender = true
            if frameLink == nil, isSetup { startDisplayLink() }
        } else {
            frameLink?.invalidate()
            frameLink = nil
            animationStartTime = nil
        }
    }

    // MARK: - Display Link

    private func startDisplayLink() {
        let link = (self as NSView).displayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        frameLink = link
    }

    @objc private func displayLinkFired() {
        if isAnimating {
            needsRender = true
        }
        renderIfNeeded()
    }

    // MARK: - Public

    /// Force a render on the next display link tick.
    public func setNeedsRender() {
        needsRender = true
    }

    // MARK: - Render

    private func renderIfNeeded() {
        guard isSetup else { return }
        guard let renderer, renderer.isReady else { return }
        guard bounds.width > 0, bounds.height > 0 else { return }
        guard needsRender else { return }
        guard let drawable = metalLayer.nextDrawable() else { return }
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return }

        needsRender = false

        // Compute animation time
        let now = CACurrentMediaTime()
        if animationStartTime == nil { animationStartTime = now }
        let elapsed = Float(now - animationStartTime!)

        // Let the material update its time-varying state
        onBeforeRender?(elapsed)

        let w = Float(bounds.width)
        let h = Float(bounds.height)

        // Orthographic projection: y-down, origin at center (matches glass convention)
        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / w, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / h, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))

        let renderDesc = MTLRenderPassDescriptor()
        renderDesc.colorAttachments[0].texture = drawable.texture
        renderDesc.colorAttachments[0].loadAction = .clear
        renderDesc.colorAttachments[0].storeAction = .store
        renderDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderDesc) else { return }

        renderer.encode(
            shape: shape,
            size: SIMD2<Float>(w, h),
            viewProjection: ortho,
            encoder: encoder
        )

        encoder.endEncoding()

        commandBuffer.commit()
        commandBuffer.waitUntilScheduled()
        drawable.present()
    }
}

#endif
