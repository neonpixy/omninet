// MaterialUIView.swift
// CrystalKit
//
// Shared iOS/visionOS UIView that hosts a CAMetalLayer and drives any MaterialRenderer.
// Mirror of MaterialMetalView for Apple's UIKit platforms.

#if canImport(UIKit)
import UIKit
import Metal
import QuartzCore
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "MaterialUIView")

@MainActor
public final class MaterialUIView: UIView {

    // MARK: - Configuration

    public var renderer: (any MaterialRenderer)? { didSet { needsRender = true } }
    public var shape: ShapeDescriptor = .roundedRect() { didSet { needsRender = true } }
    public var isAnimating: Bool = false {
        didSet { if isAnimating { needsRender = true } }
    }
    public var onBeforeRender: ((Float) -> Void)?

    // MARK: - Metal State

    private(set) var device: MTLDevice!
    private var commandQueue: MTLCommandQueue!
    private var metalLayer: CAMetalLayer!
    private var displayLink: CADisplayLink?
    private(set) var needsRender = true
    private var isSetup = false
    private var animationStartTime: CFTimeInterval?

    override public class var layerClass: AnyClass { CAMetalLayer.self }

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

        let layer = self.layer as! CAMetalLayer
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = false
        layer.isOpaque = false
        layer.backgroundColor = CGColor(gray: 0, alpha: 0)
        layer.contentsScale = UIScreen.main.scale
        metalLayer = layer

        isSetup = true
    }

    // MARK: - Layout

    override public func layoutSubviews() {
        super.layoutSubviews()
        let scale = window?.screen.scale ?? UIScreen.main.scale
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )
        needsRender = true
    }

    override public func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil {
            needsRender = true
            if displayLink == nil, isSetup { startDisplayLink() }
        } else {
            displayLink?.invalidate()
            displayLink = nil
            animationStartTime = nil
        }
    }

    // MARK: - Display Link

    private func startDisplayLink() {
        let link = CADisplayLink(target: self, selector: #selector(displayLinkFired))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    @objc private func displayLinkFired() {
        if isAnimating { needsRender = true }
        renderIfNeeded()
    }

    // MARK: - Public

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

        let now = CACurrentMediaTime()
        if animationStartTime == nil { animationStartTime = now }
        let elapsed = Float(now - animationStartTime!)

        onBeforeRender?(elapsed)

        let w = Float(bounds.width)
        let h = Float(bounds.height)

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
