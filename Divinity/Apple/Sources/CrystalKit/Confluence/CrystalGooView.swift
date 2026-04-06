// ConfluenceView.swift
// CrystalKit
//
// One Metal view per scope. All proximity groups are rendered sequentially into the
// same drawable each frame, so the view is never destroyed when groups split or merge.
// The backdrop capture, blur, SDF field, refraction UV, and composite all share the
// same coordinate space. No seams by construction.

import SwiftUI
import Metal
import QuartzCore
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "ConfluenceView")

// MARK: - SwiftUI Representable

/// SwiftUI bridge for the platform goo view.
/// One instance per scope — receives all proximity groups and renders them
/// sequentially into a single drawable each frame.
struct ConfluenceRepresentable: View {
    let groups: [ConfluenceGroup]
    let scopeSize: CGSize
    let style: FacetStyle
    let smoothK: Float
    let gazePoint: CGPoint?
    let onLuminanceUpdate: ([String], CGFloat) -> Void
    var onBackdropColorUpdate: (([String], SIMD3<Float>) -> Void)?
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    var onAFProbeResults: (([String: CGFloat]) -> Void)?

    var body: some View {
        _ConfluenceRepresentable(
            groups: groups,
            scopeSize: scopeSize,
            style: style,
            smoothK: smoothK,
            gazePoint: gazePoint,
            onLuminanceUpdate: onLuminanceUpdate,
            onBackdropColorUpdate: onBackdropColorUpdate,
            onLuminanceMaskUpdate: onLuminanceMaskUpdate,
            onAFProbeResults: onAFProbeResults
        )
    }
}

#if os(macOS)

private struct _ConfluenceRepresentable: NSViewRepresentable {
    let groups: [ConfluenceGroup]
    let scopeSize: CGSize
    let style: FacetStyle
    let smoothK: Float
    let gazePoint: CGPoint?
    let onLuminanceUpdate: ([String], CGFloat) -> Void
    var onBackdropColorUpdate: (([String], SIMD3<Float>) -> Void)?
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    var onAFProbeResults: (([String: CGFloat]) -> Void)?
    @Environment(\.bedrockProvider) var backdropProvider

    func makeNSView(context: Context) -> ConfluenceNSView {
        let view = ConfluenceNSView()
        view.onLuminanceUpdate = onLuminanceUpdate
        view.onBackdropColorUpdate = onBackdropColorUpdate
        view.onLuminanceMaskUpdate = onLuminanceMaskUpdate
        view.onAFProbeResults = onAFProbeResults
        view.backdropProvider = backdropProvider
        return view
    }

    func updateNSView(_ view: ConfluenceNSView, context: Context) {
        view.groups = groups
        view.scopeSize = scopeSize
        view.style = style
        view.smoothK = smoothK
        view.externalGazePoint = gazePoint
        view.onLuminanceUpdate = onLuminanceUpdate
        view.onBackdropColorUpdate = onBackdropColorUpdate
        view.onLuminanceMaskUpdate = onLuminanceMaskUpdate
        view.onAFProbeResults = onAFProbeResults
        if view.backdropProvider !== backdropProvider { view.backdropProvider = backdropProvider }
    }
}

// MARK: - macOS View

@MainActor
final class ConfluenceNSView: NSView {

    var groups: [ConfluenceGroup] = [] { didSet { if oldValue != groups { needsRender = true; backdropDirty = true } } }
    var scopeSize: CGSize = .zero
    var style: FacetStyle = .regular { didSet { if oldValue != style { needsRender = true; backdropDirty = true } } }
    var smoothK: Float = 40 { didSet { if oldValue != smoothK { needsRender = true } } }
    var onLuminanceUpdate: (([String], CGFloat) -> Void)?
    var onBackdropColorUpdate: (([String], SIMD3<Float>) -> Void)?
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    var onAFProbeResults: (([String: CGFloat]) -> Void)?
    var backdropProvider: (any BedrockProvider)? {
        didSet { needsRender = true; backdropDirty = true }
    }
    var externalGazePoint: CGPoint? {
        didSet {
            if oldValue != externalGazePoint { needsRender = true }
            if let point = externalGazePoint, let window {
                let windowPoint = window.convertPoint(fromScreen: point)
                cursorPosition = convert(windowPoint, from: nil)
            }
        }
    }

    private var needsRender = true
    /// Set by renderFromLiveChildren — tells the next DL tick to skip its render
    /// so the stale-groups drawable doesn't beat the live render's correct one.
    private var liveRenderActive = false
    private var cursorPosition: CGPoint?
    nonisolated(unsafe) private var mouseMonitor: Any?

    private var metalLayer: CAMetalLayer!
    private var commandQueue: MTLCommandQueue!
    private var gooRenderer: ConfluenceHolodeck!
    private var blurRenderer: FrostHolodeck!
    private var glassRenderer: Holodeck!
    private var backdropCapture: BedrockCapture!
    nonisolated(unsafe) private var frameLink: CADisplayLink?
    private var isSetup = false
    private var lastReportedLuminance: [String: CGFloat] = [:]
    private var lastReportedColor: [String: SIMD3<Float>] = [:]
    private var lastProviderGeneration: UInt64 = 0

    // Backdrop dirty flag — only re-capture when content behind the view changes.
    private var backdropDirty = true
    private var cachedBackdrop: MTLTexture?
    private var cachedRegionCopy: MTLTexture?
    private var cachedBlurred: MTLTexture?
    private var backdropObserversRegistered = false

    // Multi-pass accumulation textures for glass-on-glass depth rendering.
    // accumulationTexture: sharp backdrop + accumulated glass from previous groups.
    // accumulationBlurred: blurred version of above, used as frost input for next group.
    private var accumulationTexture: MTLTexture?
    private var accumulationBlurred: MTLTexture?
    private var accumulationSize: (Int, Int) = (0, 0)

    override init(frame: NSRect) { super.init(frame: frame); setup() }
    required init?(coder: NSCoder) { super.init(coder: coder); setup() }

    private func setup() {
        wantsLayer = true
        guard let device = MTLCreateSystemDefaultDevice(),
              let queue = device.makeCommandQueue() else {
            logger.error("ConfluenceNSView: no Metal device")
            return
        }
        commandQueue = queue
        blurRenderer = FrostHolodeck(device: device)
        gooRenderer = ConfluenceHolodeck(device: device, blurRenderer: blurRenderer)
        glassRenderer = Holodeck(device: device, blurRenderer: blurRenderer)
        backdropCapture = BedrockCapture(device: device)

        let layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = false
        layer.isOpaque = false
        layer.backgroundColor = .clear
        layer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        // Synchronize drawable presentation with Core Animation transactions.
        // Without this, the Metal content appears 1 vsync after SwiftUI moves
        // foreground content, causing visible lag during drag/scroll.
        layer.presentsWithTransaction = true
        self.layer = layer
        metalLayer = layer

        isSetup = true
        setupCursorTracking()
    }

    deinit {
        frameLink?.invalidate()
        if let m = mouseMonitor { NSEvent.removeMonitor(m) }
        NotificationCenter.default.removeObserver(self)
    }

    private func setupCursorTracking() {
        mouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged]) { [weak self] event in
            guard case .cursor = self?.style.lightSource else { return event }
            guard let self, let window, window.isVisible else { return event }
            let localPoint = convert(event.locationInWindow, from: nil)
            if let prev = cursorPosition {
                let dx = localPoint.x - prev.x
                let dy = localPoint.y - prev.y
                guard dx * dx + dy * dy > 0.25 else { return event }  // skip sub-pixel moves
            }
            cursorPosition = localPoint
            needsRender = true
            return event
        }
    }

    private func cursorWorldPos() -> SIMD2<Float> {
        guard case .cursor = style.lightSource, let cursor = cursorPosition else {
            return SIMD2<Float>(-.infinity, -.infinity)
        }
        // NSView: y increases upward. Shader world coords: y-down, (0,0) at group center.
        let wx = Float(cursor.x - bounds.width * 0.5)
        let wy = Float((bounds.height - cursor.y) - bounds.height * 0.5)
        return SIMD2<Float>(wx, wy)
    }

    override func layout() {
        super.layout()
        guard let metalLayer else { return }
        let scale = window?.backingScaleFactor ?? 2.0
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(width: bounds.width * scale, height: bounds.height * scale)
        backdropDirty = true
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            ConfluenceLiveChildStore.activeGooView = self
            metalLayer?.contentsScale = window?.backingScaleFactor ?? 2.0
            needsRender = true
            backdropDirty = true
            if frameLink == nil, isSetup { startDisplayLink() }
            registerBackdropObservers()
        } else {
            if ConfluenceLiveChildStore.activeGooView === self {
                ConfluenceLiveChildStore.activeGooView = nil
            }
            frameLink?.invalidate()
            frameLink = nil
            unregisterBackdropObservers()
        }
    }

    private static let canvasNeedsRenderName = Notification.Name("metalCanvasNeedsRender")

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

    private func startDisplayLink() {
        let link = (self as NSView).displayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        frameLink = link
    }

    private var maskFrameCounter: UInt64 = 0

    @objc private func tick() {
        // Clear stale live frames each tick so only current-frame onChange data is used.
        // Skip during interactions — the TimelineView-driven patchedConfluenceGroups needs
        // the live store to persist between display link ticks. Without this, the
        // store is empty when patchedConfluenceGroups evaluates, causing it to fall back to
        // stale @State children (original pre-drag positions), which produces flicker.
        if !ConfluenceLiveChildStore.interactionsActive {
            ConfluenceLiveChildStore.children.removeAll(keepingCapacity: true)
        }
        // Force backdrop refresh for dynamic content (video, animation behind glass).
        let hasDynamic = style.dynamicBackdrop
            || groups.flatMap(\.children).contains(where: \.style.dynamicBackdrop)
        if hasDynamic {
            backdropDirty = true
            needsRender = true
        }
        // During active drag, renderFromLiveChildren sets liveRenderActive.
        // Skip the DL render so its stale-groups drawable doesn't beat the
        // live render's correct one (presentsWithTransaction shows the first
        // drawable queued in a CA transaction, not the last).
        if liveRenderActive {
            liveRenderActive = false
            return
        }
        render()
    }

    private func render() {
        // Check if the backdrop provider has new content we haven't captured yet.
        if let provider = backdropProvider,
           provider.contentGeneration != lastProviderGeneration {
            backdropDirty = true
            needsRender = true
        }

        guard needsRender else { return }
        guard isSetup, gooRenderer.isReady else { return }
        guard bounds.width > 0, bounds.height > 0 else { return }
        guard let window, window.isVisible else { return }

        // Patch groups with live frame data from the live store.
        // The live store is updated synchronously from onChange (after SwiftUI
        // layout) and always has frames at least as fresh as `groups` (which
        // arrive one frame late via the preference → @State pipeline).
        // This ensures every render — whether triggered by the display link
        // or by renderFromLiveChildren — uses the best available positions.
        let renderGroups: [ConfluenceGroup]
        let liveChildren = ConfluenceLiveChildStore.children
        if !liveChildren.isEmpty {
            renderGroups = groups.map { group in
                let patched = group.children.map { child -> ConfluenceChildInfo in
                    guard let live = liveChildren[child.id] else { return child }
                    var info = ConfluenceChildInfo(
                        id: child.id,
                        frame: live.frame,
                        style: child.style,
                        shape: child.shape,
                        crystallized: child.crystallized,
                        zIndex: live.zIndex
                    )
                    // Prefer the interaction engine's z-index (most current).
                    if let z = ConfluenceLiveChildStore.interactionZIndices[child.id] {
                        info.zIndex = z
                    }
                    return info
                }
                return ConfluenceGroup(
                    id: group.id,
                    childIDs: group.childIDs,
                    children: patched,
                    boundingBox: group.boundingBox
                )
            }
        } else {
            renderGroups = groups
        }

        let allChildren = renderGroups.flatMap(\.children)
        guard !allChildren.isEmpty else { return }

        guard let drawable = metalLayer.nextDrawable() else { return }
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return }

        needsRender = false

        let gw = Float(bounds.width)
        let gh = Float(bounds.height)
        let groupSize = SIMD2<Float>(gw, gh)

        // Capture backdrop only when dirty — skip capture on idle frames.
        let backdropChanged = backdropDirty
        if backdropDirty {
            backdropDirty = false

            // Prefer the backdrop provider (pre-rendered colorful texture) over
            // CALayer.render, which can't capture Metal-rendered SwiftUI content.
            let backdrop: MTLTexture?
            if let provider = backdropProvider {
                let gen = provider.contentGeneration
                if gen != lastProviderGeneration || cachedBackdrop == nil {
                    lastProviderGeneration = gen
                    backdrop = provider.backdropTexture(for: .zero, scale: window.backingScaleFactor)
                } else {
                    backdrop = cachedBackdrop
                }
            } else {
                backdrop = backdropCapture.capture(behind: self)
            }

            if let backdrop {
                cachedBackdrop = backdrop
                updateLuminance(from: backdrop)

                let avgFrost = allChildren.map { Float($0.style.frost) }.reduce(0, +) / Float(allChildren.count)
                let avgShapeMinDim = allChildren.map { CGFloat(min($0.frame.width, $0.frame.height)) }.reduce(0, +) / CGFloat(allChildren.count)
                let blurRadius = Holodeck.blurRadius(
                    forFrost: CGFloat(avgFrost),
                    nodeSize: CGSize(width: avgShapeMinDim, height: avgShapeMinDim)
                )
                let fullRegion = MTLRegionMake2D(0, 0, backdrop.width, backdrop.height)
                if let regionCopy = blurRenderer.copyRegion(from: backdrop, region: fullRegion, commandBuffer: commandBuffer) {
                    cachedRegionCopy = regionCopy
                    cachedBlurred = (blurRadius > 0
                        ? blurRenderer.blur(source: regionCopy, radius: blurRadius, commandBuffer: commandBuffer)
                        : regionCopy) ?? regionCopy
                }

            }
        }

        guard let regionCopy = cachedRegionCopy, let blurred = cachedBlurred else { return }

        // macOS: frames are scope-relative (top-left origin). Convert to center-origin.
        let scopeCenter = CGPoint(x: bounds.width * 0.5, y: bounds.height * 0.5)

        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / gw, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / gh, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))

        struct GroupRenderData {
            let shapes: [ConfluenceShapeDescriptor]
            let uniforms: ConfluenceGroupUniforms
            let sdfTextures: [MTLTexture?]
        }
        var groupData: [GroupRenderData] = []

        for group in renderGroups {
            // Crystallized groups use the child's own style instead of the scope style.
            let isCrystallized = group.children.count == 1
                && group.children[0].crystallized
            let groupStyle = isCrystallized ? group.children[0].style : style
            let groupSmoothK = isCrystallized ? Float(0) : smoothK

            var sdfTextures: [MTLTexture?] = []
            var shapes: [ConfluenceShapeDescriptor] = []
            for child in group.children {
                let hasSDF = child.shape.sdfTexture != nil
                let texIdx: Int32 = hasSDF ? Int32(sdfTextures.count) : -1
                shapes.append(ConfluenceHolodeck.buildShapeDescriptor(
                    child: child, groupCenter: scopeCenter, style: groupStyle,
                    sdfTextureIndex: texIdx
                ))
                sdfTextures.append(child.shape.sdfTexture)
            }

            var uniforms = ConfluenceHolodeck.buildGroupUniforms(
                style: groupStyle,
                groupSize: groupSize,
                cursorWorldPos: cursorWorldPos(),
                shapeCount: UInt32(shapes.count),
                smoothK: groupSmoothK
            )
            uniforms.blurPadding = .zero
            uniforms.captureHalfExtent = groupSize * 0.5
            uniforms.captureOffset = .zero

            // Resolve appearance for the contrast layer in the shader.
            if groupStyle.appearance == .light {
                uniforms.appearanceMode = 1
            } else if groupStyle.appearance == .dark {
                uniforms.appearanceMode = 2
            } else if groupStyle.appearance == .auto {
                let isDark = self.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                uniforms.appearanceMode = isDark ? 2 : 1
            } else {
                uniforms.appearanceMode = 0  // .base and unknown custom appearances
            }

            groupData.append(GroupRenderData(
                shapes: shapes, uniforms: uniforms,
                sdfTextures: sdfTextures
            ))
        }

        guard !groupData.isEmpty else { return }

        // Sort groupData by the corresponding group's sortKey (back-to-front).
        let sortedGroupData: [(ConfluenceGroup, GroupRenderData)]
        let paired = zip(renderGroups, groupData).map { ($0, $1) }
        let sorted = paired.sorted { ($0.0.sortKey, $0.0.id) < ($1.0.sortKey, $1.0.id) }
        sortedGroupData = sorted

        // Multi-pass: when multiple groups have crystallized children with distinct
        // sort keys, render each group against an accumulating backdrop so front
        // panels refract through back panels' glass surfaces.
        let hasCrystallized = renderGroups.contains { $0.children.contains(where: \.crystallized) }
        let distinctSortKeys = Set(renderGroups.map(\.sortKey)).count > 1
        let needsMultiPass = groupData.count > 1 && hasCrystallized && distinctSortKeys

        if needsMultiPass {
            ensureAccumulationTexture(width: regionCopy.width, height: regionCopy.height,
                                      device: metalLayer.device!)

            guard let accTex = accumulationTexture, let accBlur = accumulationBlurred else { return }

            // Seed accumulation with the original backdrop.
            guard let seedBlit = commandBuffer.makeBlitCommandEncoder() else { return }
            let texSize = MTLSize(width: regionCopy.width, height: regionCopy.height, depth: 1)
            seedBlit.copy(from: regionCopy, sourceSlice: 0, sourceLevel: 0,
                          sourceOrigin: .init(x: 0, y: 0, z: 0), sourceSize: texSize,
                          to: accTex, destinationSlice: 0, destinationLevel: 0,
                          destinationOrigin: .init(x: 0, y: 0, z: 0))
            seedBlit.copy(from: blurred, sourceSlice: 0, sourceLevel: 0,
                          sourceOrigin: .init(x: 0, y: 0, z: 0), sourceSize: texSize,
                          to: accBlur, destinationSlice: 0, destinationLevel: 0,
                          destinationOrigin: .init(x: 0, y: 0, z: 0))
            seedBlit.endEncoding()

            // Clear drawable, then render each group with .load to accumulate.
            let clearDesc = MTLRenderPassDescriptor()
            clearDesc.colorAttachments[0].texture = drawable.texture
            clearDesc.colorAttachments[0].loadAction = .clear
            clearDesc.colorAttachments[0].storeAction = .store
            clearDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
            if let clearEncoder = commandBuffer.makeRenderCommandEncoder(descriptor: clearDesc) {
                clearEncoder.endEncoding()
            }

            for (index, (_, data)) in sortedGroupData.enumerated() {
                // Per-group blur: use this group's average frost for its backdrop blur.
                let groupFrost = data.shapes.isEmpty ? Float(0)
                    : data.shapes.map(\.frost).reduce(0, +) / Float(data.shapes.count)
                let groupChildren = sortedGroupData[index].0.children
                let groupMinDim = groupChildren.map { CGFloat(min($0.frame.width, $0.frame.height)) }.reduce(0, +)
                    / max(CGFloat(groupChildren.count), 1)
                let groupBlurRadius = Holodeck.blurRadius(
                    forFrost: CGFloat(groupFrost),
                    nodeSize: CGSize(width: groupMinDim, height: groupMinDim)
                )

                // Use per-group blurred accumulation if frost differs from global.
                let groupBlurred: MTLTexture
                if groupBlurRadius > 0, index > 0 {
                    // Re-blur the accumulation texture at this group's frost level.
                    groupBlurred = blurRenderer.blur(source: accTex, radius: groupBlurRadius, commandBuffer: commandBuffer) ?? accBlur
                } else {
                    groupBlurred = accBlur
                }

                // Render each group into drawable with premultiplied blending (load existing).
                let passDesc = MTLRenderPassDescriptor()
                passDesc.colorAttachments[0].texture = drawable.texture
                passDesc.colorAttachments[0].loadAction = .load
                passDesc.colorAttachments[0].storeAction = .store
                guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDesc) else { continue }

                gooRenderer.encodeComposite(
                    blurredBackground: groupBlurred,
                    sharpBackground: accTex,
                    shapes: data.shapes,
                    uniforms: data.uniforms,
                    sdfTextures: data.sdfTextures,
                    groupCenter: .zero,
                    groupSize: groupSize,
                    viewportUniforms: HolodeckViewportUniforms(viewProjection: ortho),
                    encoder: encoder
                )
                encoder.endEncoding()

                // Composite this group's output onto the accumulation texture
                // so the next group sees it in its backdrop.
                if index < sortedGroupData.count - 1 {
                    gooRenderer.encodeOverComposite(
                        source: drawable.texture,
                        onto: accTex,
                        groupSize: groupSize,
                        commandBuffer: commandBuffer
                    )
                }
            }
        } else {
            // Single-pass: original path — all groups into one encoder.
            let passDesc = MTLRenderPassDescriptor()
            passDesc.colorAttachments[0].texture = drawable.texture
            passDesc.colorAttachments[0].loadAction = .clear
            passDesc.colorAttachments[0].storeAction = .store
            passDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
            guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDesc) else { return }

            for (_, data) in sortedGroupData {
                gooRenderer.encodeComposite(
                    blurredBackground: blurred,
                    sharpBackground: regionCopy,
                    shapes: data.shapes,
                    uniforms: data.uniforms,
                    sdfTextures: data.sdfTextures,
                    groupCenter: .zero,
                    groupSize: groupSize,
                    viewportUniforms: HolodeckViewportUniforms(viewProjection: ortho),
                    encoder: encoder
                )
            }

            encoder.endEncoding()
        }

        // Generate luminance mask for adaptive foreground when the backdrop changed,
        // or periodically for dynamic backdrops (~every 66ms at 60fps).
        let needsAdaptiveForeground = groups.flatMap(\.children).contains { $0.style.needsLuminanceMask }
            || style.needsLuminanceMask
        maskFrameCounter &+= 1
        let hasDynamicBackdrop = style.dynamicBackdrop
            || groups.flatMap(\.children).contains(where: \.style.dynamicBackdrop)
        let shouldRegenerateMask = backdropChanged
            || (hasDynamicBackdrop && maskFrameCounter % 4 == 0)
        if needsAdaptiveForeground && shouldRegenerateMask {
            let scale = Float(metalLayer?.contentsScale ?? 2.0)
            let viewSize = CGSize(width: bounds.width, height: bounds.height)
            let avgFrostForMask = allChildren.isEmpty ? Float(style.frost)
                : allChildren.map { Float($0.style.frost) }.reduce(0, +) / Float(allChildren.count)
            let maskTex = glassRenderer.encodeLuminanceMask(
                backgroundTexture: regionCopy,  // already unwrapped from guard above
                viewSize: viewSize,
                backingScale: scale,
                frost: avgFrostForMask,
                tintColor: premultipliedRGBA(style.tintColor),
                tintOpacity: Float(style.tintOpacity),
                commandBuffer: commandBuffer
            )
            if let maskTex, maskTex.storageMode == MTLStorageMode.managed {
                if let blit = commandBuffer.makeBlitCommandEncoder() {
                    blit.synchronize(resource: maskTex)
                    blit.endEncoding()
                }
            }

            // GPU-side AF probe: sample the mask for all children in one dispatch.
            // Children's frames are scope-relative; the mask covers the full scope.
            var probeBuffer: MTLBuffer?
            var probeChildIDs: [String] = []
            if let maskTex, onAFProbeResults != nil {
                let scopeFrame = CGRect(origin: .zero, size: CGSize(width: CGFloat(gw), height: CGFloat(gh)))
                var elements: [(center: CGPoint, size: CGSize)] = []
                for child in allChildren {
                    probeChildIDs.append(child.id)
                    elements.append((
                        center: CGPoint(x: child.frame.midX, y: child.frame.midY),
                        size: CGSize(width: child.frame.width, height: child.frame.height)
                    ))
                }
                probeBuffer = glassRenderer.encodeAFProbe(
                    maskTexture: maskTex,
                    elements: elements,
                    glassFrame: scopeFrame,
                    commandBuffer: commandBuffer
                )
            }

            nonisolated(unsafe) let sendableTex = maskTex
            nonisolated(unsafe) let sendableProbeBuffer = probeBuffer
            let capturedChildIDs = probeChildIDs
            commandBuffer.addCompletedHandler { [weak self] _ in
                DispatchQueue.main.async {
                    self?.onLuminanceMaskUpdate?(sendableTex)
                    // Publish GPU probe results keyed by child ID.
                    if let buf = sendableProbeBuffer {
                        let ptr = buf.contents().bindMemory(to: Float.self, capacity: capturedChildIDs.count)
                        var results: [String: CGFloat] = [:]
                        for (i, childID) in capturedChildIDs.enumerated() {
                            results[childID] = CGFloat(ptr[i])
                        }
                        self?.onAFProbeResults?(results)
                    }
                }
            }
        }

        commandBuffer.commit()
        commandBuffer.waitUntilScheduled()
        drawable.present()
    }

    /// Triggers an immediate Metal re-render with live frame data from the store.
    /// Called by `ConfluenceLiveChildStore.update()` during `onChange(of: reportedFrame)` —
    /// after SwiftUI layout completes but within the same Core Animation transaction.
    /// The live data is integrated into `render()` itself, so this just marks dirty and renders.
    /// Also sets `liveRenderActive` so the next DL tick skips its stale render.
    func renderFromLiveChildren() {
        liveRenderActive = true
        needsRender = true
        render()
    }

    private func updateLuminance(from texture: MTLTexture) {
        guard onLuminanceUpdate != nil || onBackdropColorUpdate != nil else { return }
        let tw = texture.width, th = texture.height
        guard tw > 0, th > 0 else { return }
        let bw = bounds.width, bh = bounds.height
        guard bw > 0, bh > 0 else { return }

        var pixel = [UInt8](repeating: 0, count: 4)
        let bytesPerRow = tw * 4

        for group in groups {
            for child in group.children {
                // Child frames are in scope-local coords; texture covers full scope.
                let pixelX = Int((child.frame.midX / bw) * CGFloat(tw))
                let pixelY = Int((child.frame.midY / bh) * CGFloat(th))
                let cx = max(0, min(pixelX, tw - 1))
                let cy = max(0, min(pixelY, th - 1))

                // Center probe for color — single point preserves backdrop saturation.
                // Averaging spread probes kills chroma when backdrop is multicolored.
                texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                                 from: MTLRegionMake2D(cx, cy, 1, 1), mipmapLevel: 0)
                let centerR = Float(pixel[2]) / 255, centerG = Float(pixel[1]) / 255, centerB = Float(pixel[0]) / 255
                let centerColor = SIMD3<Float>(centerR, centerG, centerB)

                // 5-probe average for luminance — spatial spread is fine for brightness.
                let sx = max(Int(child.frame.width / bw * CGFloat(tw) * 0.3), max(tw * 3 / 100, 1))
                let sy = max(Int(child.frame.height / bh * CGFloat(th) * 0.3), max(th * 3 / 100, 1))
                let probes = [
                    (cx, cy),
                    (min(cx + sx, tw - 1), cy), (max(cx - sx, 0), cy),
                    (cx, min(cy + sy, th - 1)), (cx, max(cy - sy, 0))
                ]

                var totalLum: Float = 0
                for (px, py) in probes {
                    texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                                     from: MTLRegionMake2D(px, py, 1, 1), mipmapLevel: 0)
                    let r = Float(pixel[2]) / 255, g = Float(pixel[1]) / 255, b = Float(pixel[0]) / 255
                    totalLum += 0.2126 * r + 0.7152 * g + 0.0722 * b
                }

                let avg = CGFloat(totalLum / 5)
                let prevLum = lastReportedLuminance[child.id] ?? 0.5
                if abs(avg - prevLum) > 0.05 {
                    lastReportedLuminance[child.id] = avg
                    onLuminanceUpdate?([child.id], avg)
                }

                let prevColor = lastReportedColor[child.id] ?? SIMD3(repeating: 0.5)
                if simd_length(centerColor - prevColor) > 0.05 {
                    lastReportedColor[child.id] = centerColor
                    onBackdropColorUpdate?([child.id], centerColor)
                }
            }
        }
    }

    private func ensureAccumulationTexture(width: Int, height: Int, device: MTLDevice) {
        guard (width, height) != accumulationSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        desc.storageMode = .private

        accumulationTexture = device.makeTexture(descriptor: desc)
        accumulationTexture?.label = "CrystalKit Accumulation Sharp"
        accumulationBlurred = device.makeTexture(descriptor: desc)
        accumulationBlurred?.label = "CrystalKit Accumulation Blurred"
        accumulationSize = (width, height)
    }

}

#else

// MARK: - iOS View

private struct _ConfluenceRepresentable: UIViewRepresentable {
    let groups: [ConfluenceGroup]
    let scopeSize: CGSize
    let style: FacetStyle
    let smoothK: Float
    let gazePoint: CGPoint?
    let onLuminanceUpdate: ([String], CGFloat) -> Void
    var onBackdropColorUpdate: (([String], SIMD3<Float>) -> Void)?
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    var onAFProbeResults: (([String: CGFloat]) -> Void)?
    @Environment(\.bedrockProvider) var backdropProvider

    func makeUIView(context: Context) -> ConfluenceUIView {
        let view = ConfluenceUIView()
        view.onLuminanceUpdate = onLuminanceUpdate
        view.onBackdropColorUpdate = onBackdropColorUpdate
        view.onLuminanceMaskUpdate = onLuminanceMaskUpdate
        view.onAFProbeResults = onAFProbeResults
        view.backdropProvider = backdropProvider
        return view
    }

    func updateUIView(_ view: ConfluenceUIView, context: Context) {
        view.groups = groups
        view.scopeSize = scopeSize
        view.style = style
        view.smoothK = smoothK
        view.externalGazePoint = gazePoint
        view.onLuminanceUpdate = onLuminanceUpdate
        view.onBackdropColorUpdate = onBackdropColorUpdate
        view.onLuminanceMaskUpdate = onLuminanceMaskUpdate
        view.onAFProbeResults = onAFProbeResults
        if view.backdropProvider !== backdropProvider { view.backdropProvider = backdropProvider }
    }
}

@MainActor
final class ConfluenceUIView: UIView {

    override class var layerClass: AnyClass { CAMetalLayer.self }

    var groups: [ConfluenceGroup] = [] { didSet { if oldValue != groups { needsRender = true; backdropDirty = true } } }
    var scopeSize: CGSize = .zero
    var style: FacetStyle = .regular { didSet { if oldValue != style { needsRender = true; backdropDirty = true } } }
    var smoothK: Float = 40 { didSet { if oldValue != smoothK { needsRender = true } } }
    var onLuminanceUpdate: (([String], CGFloat) -> Void)?
    var onBackdropColorUpdate: (([String], SIMD3<Float>) -> Void)?
    var onLuminanceMaskUpdate: ((MTLTexture?) -> Void)?
    var onAFProbeResults: (([String: CGFloat]) -> Void)?
    var backdropProvider: (any BedrockProvider)? {
        didSet { needsRender = true; backdropDirty = true }
    }
    var externalGazePoint: CGPoint? { didSet { if oldValue != externalGazePoint { needsRender = true } } }

    private var cursorPosition: CGPoint?
    private var needsRender = true
    private var lastTiltDirection: CGPoint?
    private var lastHoverPoint: CGPoint?
    private var metalLayer: CAMetalLayer! { layer as? CAMetalLayer }
    private var commandQueue: MTLCommandQueue!
    private var gooRenderer: ConfluenceHolodeck!
    private var blurRenderer: FrostHolodeck!
    private var glassRenderer: Holodeck!
    private var backdropCapture: BedrockCapture!
    nonisolated(unsafe) private var displayLink: CADisplayLink?
    private var isSetup = false
    private var lastReportedLuminance: [String: CGFloat] = [:]
    private var lastReportedColor: [String: SIMD3<Float>] = [:]
    private var lastProviderGeneration: UInt64 = 0

    // Backdrop dirty flag — only re-capture when content behind the view changes.
    private var backdropDirty = true
    private var cachedBackdrop: MTLTexture?
    private var cachedRegionCopy: MTLTexture?
    private var cachedBlurred: MTLTexture?

    // Multi-pass accumulation textures for glass-on-glass depth rendering.
    private var accumulationTexture: MTLTexture?
    private var accumulationBlurred: MTLTexture?
    private var accumulationSize: (Int, Int) = (0, 0)

    /// Offscreen texture for the goo composite output. Written to
    /// `ConfluenceOutputStore.texture` each frame so standalone glass views
    /// (which may be siblings in the view tree) can composite it into their
    /// captured backdrop. `.shared` storage for CPU readability.
    private var offscreenTexture: MTLTexture?
    private var offscreenSize: (Int, Int) = (0, 0)

    override init(frame: CGRect) { super.init(frame: frame); setup() }
    required init?(coder: NSCoder) { super.init(coder: coder); setup() }

    private func setup() {
        backgroundColor = .clear
        guard let device = MTLCreateSystemDefaultDevice(),
              let queue = device.makeCommandQueue() else { return }
        commandQueue = queue
        blurRenderer = FrostHolodeck(device: device)
        gooRenderer = ConfluenceHolodeck(device: device, blurRenderer: blurRenderer)
        glassRenderer = Holodeck(device: device, blurRenderer: blurRenderer)
        backdropCapture = BedrockCapture(device: device)

        let ml = metalLayer!
        ml.device = device
        ml.pixelFormat = .bgra8Unorm
        ml.framebufferOnly = false
        ml.isOpaque = false
        ml.backgroundColor = UIColor.clear.cgColor
        ml.contentsScale = UIScreen.main.scale
        ml.presentsWithTransaction = true
        isSetup = true

        setupHover()

        #if !os(visionOS)
        GleamTiltTracker.shared.addConsumer()
        GleamHoverTracker.shared.addConsumer()
        #endif
    }

    deinit {
        displayLink?.invalidate()
        #if !os(visionOS)
        MainActor.assumeIsolated {
            GleamTiltTracker.shared.removeConsumer()
            GleamHoverTracker.shared.removeConsumer()
        }
        #endif
    }

    private func setupHover() {
        #if !os(tvOS)
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(handleHover(_:)))
        addGestureRecognizer(hover)
        #endif
    }

    #if !os(tvOS)
    @objc private func handleHover(_ r: UIHoverGestureRecognizer) {
        guard case .cursor = style.lightSource else { return }
        cursorPosition = r.location(in: self)
    }
    #endif

    private func cursorWorldPos() -> SIMD2<Float> {
        guard case .cursor = style.lightSource else {
            return SIMD2<Float>(-.infinity, -.infinity)
        }

        // Tilt tracker: only the direction matters — magnitude is irrelevant.
        // The shader uses cursorActive > 1.5 to detect directional mode and
        // switches to normalDotLight-based rim lighting (no proximity falloff).
        // We just need a finite vector pointing the right way.
        if let dir = GleamTiltTracker._sharedLightDirection {
            return SIMD2<Float>(Float(dir.x), Float(dir.y))
        }

        // External gaze/light point (screen coords → view-local)
        if let lightScreen = externalGazePoint, let window {
            let windowPoint = window.convert(lightScreen, from: nil)
            let local = convert(windowPoint, from: window)
            return SIMD2<Float>(Float(local.x - bounds.width * 0.5), Float(local.y - bounds.height * 0.5))
        }

        // iPad hover tracker (global window-level hover capture)
        if let hoverScreen = GleamHoverTracker._sharedHoverPoint, let window {
            let windowPoint = window.convert(hoverScreen, from: nil)
            let local = convert(windowPoint, from: window)
            return SIMD2<Float>(Float(local.x - bounds.width * 0.5), Float(local.y - bounds.height * 0.5))
        }

        // Fall back to per-view hover cursor tracking
        guard let p = cursorPosition else {
            return SIMD2<Float>(-.infinity, -.infinity)
        }
        return SIMD2<Float>(Float(p.x - bounds.width * 0.5), Float(p.y - bounds.height * 0.5))
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let scale = UIScreen.main.scale
        // metalLayer IS self.layer (via layerClass) — no frame assignment needed.
        metalLayer.contentsScale = scale
        metalLayer.drawableSize = CGSize(width: bounds.width * scale, height: bounds.height * scale)
        backdropDirty = true
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        backdropDirty = true
        if window != nil {
            if displayLink == nil, isSetup { startDisplayLink() }
        } else {
            displayLink?.invalidate()
            displayLink = nil
        }
    }

    private func startDisplayLink() {
        let link = CADisplayLink(target: self, selector: #selector(tick))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    private var maskFrameCounter: UInt64 = 0

    @objc private func tick() {
        // Cursor/tilt light changes affect rim lighting — mark dirty only when value changed.
        if case .cursor = style.lightSource {
            let currentTilt = GleamTiltTracker._sharedLightDirection
            let currentHover = GleamHoverTracker._sharedHoverPoint
            if currentTilt != lastTiltDirection || currentHover != lastHoverPoint {
                lastTiltDirection = currentTilt
                lastHoverPoint = currentHover
                needsRender = true
            }
        }
        // Force backdrop refresh for dynamic content (video, animation behind glass).
        let hasDynamic = style.dynamicBackdrop
            || groups.flatMap(\.children).contains(where: \.style.dynamicBackdrop)
        if hasDynamic {
            backdropDirty = true
            needsRender = true
        }
        // Check if the backdrop provider has new content we haven't captured yet.
        if let provider = backdropProvider,
           provider.contentGeneration != lastProviderGeneration {
            backdropDirty = true
            needsRender = true
        }
        guard needsRender else { return }
        render()
    }

    private func render() {
        guard isSetup, gooRenderer.isReady else { return }
        guard bounds.width > 0, bounds.height > 0 else { return }

        let allChildren = groups.flatMap(\.children)
        guard !allChildren.isEmpty else { return }

        needsRender = false

        guard let drawable = metalLayer.nextDrawable() else { return }
        guard let commandBuffer = commandQueue.makeCommandBuffer() else { return }

        // Derive point-size from the drawable to stay in sync with the pixel grid.
        let dt = drawable.texture
        let scale = metalLayer.contentsScale
        let gw = Float(dt.width) / Float(scale)
        let gh = Float(dt.height) / Float(scale)
        let groupSize = SIMD2<Float>(gw, gh)

        // Capture backdrop only when dirty — skip CALayer.render on idle frames.
        let backdropChanged = backdropDirty
        if backdropDirty {
            backdropDirty = false

            // Prefer the backdrop provider (pre-rendered colorful texture) over
            // CALayer.render, which can't capture Metal-rendered SwiftUI content.
            let backdrop: MTLTexture?
            if let provider = backdropProvider {
                let gen = provider.contentGeneration
                if gen != lastProviderGeneration || cachedBackdrop == nil {
                    lastProviderGeneration = gen
                    backdrop = provider.backdropTexture(for: .zero, scale: metalLayer.contentsScale)
                } else {
                    backdrop = cachedBackdrop
                }
            } else {
                backdrop = backdropCapture.capture(behind: self)
            }

            if let backdrop {
                cachedBackdrop = backdrop
                updateLuminance(from: backdrop)

                // Blur uses average frost across ALL children (shared backdrop).
                let avgFrost = allChildren.map { Float($0.style.frost) }.reduce(0, +) / Float(allChildren.count)
                let avgShapeMinDim = allChildren.map { CGFloat(min($0.frame.width, $0.frame.height)) }.reduce(0, +) / CGFloat(allChildren.count)
                let blurRadius = Holodeck.blurRadius(
                    forFrost: CGFloat(avgFrost),
                    nodeSize: CGSize(width: avgShapeMinDim, height: avgShapeMinDim)
                )
                let fullRegion = MTLRegionMake2D(0, 0, backdrop.width, backdrop.height)
                if let regionCopy = blurRenderer.copyRegion(from: backdrop, region: fullRegion, commandBuffer: commandBuffer) {
                    cachedRegionCopy = regionCopy
                    cachedBlurred = (blurRadius > 0
                        ? blurRenderer.blur(source: regionCopy, radius: blurRadius, commandBuffer: commandBuffer)
                        : regionCopy) ?? regionCopy
                }

            }
        }

        guard let regionCopy = cachedRegionCopy, let blurred = cachedBlurred else { return }

        // Use drawable-derived size for center so the projection and shape
        // positions use the exact same coordinate system as the pixel grid.
        let scopeCenter = CGPoint(x: CGFloat(gw) * 0.5, y: CGFloat(gh) * 0.5)

        // On iPhone, modulate light intensity by ambient screen brightness.
        var renderStyle = style
        if GleamTiltTracker._sharedLightDirection != nil {
            renderStyle.lightIntensity *= GleamTiltTracker._sharedBrightnessIntensity
        }

        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / gw, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / gh, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))

        struct GroupRenderData {
            let shapes: [ConfluenceShapeDescriptor]
            let uniforms: ConfluenceGroupUniforms
            let sdfTextures: [MTLTexture?]
        }
        var groupData: [GroupRenderData] = []

        for group in groups {
            // Crystallized groups use the child's own style instead of the scope style.
            let isCrystallized = group.children.count == 1
                && group.children[0].crystallized
            let groupStyle = isCrystallized ? group.children[0].style : renderStyle
            let groupSmoothK = isCrystallized ? Float(0) : smoothK

            // Convert children from global (window) coords to this view's local coords.
            let localChildren: [ConfluenceChildInfo] = group.children.map { child in
                let globalMid = CGPoint(x: child.frame.midX, y: child.frame.midY)
                let localMid = self.convert(globalMid, from: nil)
                let localFrame = CGRect(
                    x: localMid.x - child.frame.width * 0.5,
                    y: localMid.y - child.frame.height * 0.5,
                    width: child.frame.width,
                    height: child.frame.height
                )
                return ConfluenceChildInfo(
                    id: child.id, frame: localFrame,
                    style: child.style, shape: child.shape,
                    crystallized: child.crystallized
                )
            }

            var sdfTextures: [MTLTexture?] = []
            var shapes: [ConfluenceShapeDescriptor] = []
            for child in localChildren {
                let hasSDF = child.shape.sdfTexture != nil
                let texIdx: Int32 = hasSDF ? Int32(sdfTextures.count) : -1
                shapes.append(ConfluenceHolodeck.buildShapeDescriptor(
                    child: child, groupCenter: scopeCenter, style: groupStyle,
                    sdfTextureIndex: texIdx
                ))
                sdfTextures.append(child.shape.sdfTexture)
            }

            var uniforms = ConfluenceHolodeck.buildGroupUniforms(
                style: groupStyle,
                groupSize: groupSize,
                cursorWorldPos: cursorWorldPos(),
                shapeCount: UInt32(shapes.count),
                smoothK: groupSmoothK
            )
            if let tiltDir = GleamTiltTracker._sharedLightDirection, uniforms.cursorActive > 0.5 {
                uniforms.cursorActive = 2.0
                uniforms.tiltY = Float(tiltDir.y)
            }
            uniforms.blurPadding = .zero
            uniforms.captureHalfExtent = groupSize * 0.5
            uniforms.captureOffset = .zero

            // Resolve appearance for the contrast layer in the shader.
            if groupStyle.appearance == .light {
                uniforms.appearanceMode = 1
            } else if groupStyle.appearance == .dark {
                uniforms.appearanceMode = 2
            } else if groupStyle.appearance == .auto {
                let isDark = self.traitCollection.userInterfaceStyle == .dark
                uniforms.appearanceMode = isDark ? 2 : 1
            } else {
                uniforms.appearanceMode = 0  // .base and unknown custom appearances
            }

            groupData.append(GroupRenderData(
                shapes: shapes, uniforms: uniforms,
                sdfTextures: sdfTextures
            ))
        }

        guard !groupData.isEmpty else { return }

        // Sort groupData by the corresponding group's sortKey (back-to-front).
        let sortedGroupData: [(ConfluenceGroup, GroupRenderData)]
        let paired = zip(groups, groupData).map { ($0, $1) }
        let sorted = paired.sorted { ($0.0.sortKey, $0.0.id) < ($1.0.sortKey, $1.0.id) }
        sortedGroupData = sorted

        // Multi-pass: when multiple groups have crystallized children with distinct
        // sort keys, render each group against an accumulating backdrop.
        let hasCrystallized = groups.contains { $0.children.contains(where: \.crystallized) }
        let distinctSortKeys = Set(groups.map(\.sortKey)).count > 1
        let needsMultiPass = groupData.count > 1 && hasCrystallized && distinctSortKeys

        // Ensure offscreen texture matches the drawable size.
        let drawableW = drawable.texture.width
        let drawableH = drawable.texture.height
        ensureOffscreenTexture(width: drawableW, height: drawableH)
        guard let offscreen = offscreenTexture else { return }

        if needsMultiPass {
            ensureAccumulationTexture(width: regionCopy.width, height: regionCopy.height,
                                      device: metalLayer.device!)
            guard let accTex = accumulationTexture, let accBlur = accumulationBlurred else { return }

            // Seed accumulation with the original backdrop.
            guard let seedBlit = commandBuffer.makeBlitCommandEncoder() else { return }
            let texSize = MTLSize(width: regionCopy.width, height: regionCopy.height, depth: 1)
            seedBlit.copy(from: regionCopy, sourceSlice: 0, sourceLevel: 0,
                          sourceOrigin: .init(x: 0, y: 0, z: 0), sourceSize: texSize,
                          to: accTex, destinationSlice: 0, destinationLevel: 0,
                          destinationOrigin: .init(x: 0, y: 0, z: 0))
            seedBlit.copy(from: blurred, sourceSlice: 0, sourceLevel: 0,
                          sourceOrigin: .init(x: 0, y: 0, z: 0), sourceSize: texSize,
                          to: accBlur, destinationSlice: 0, destinationLevel: 0,
                          destinationOrigin: .init(x: 0, y: 0, z: 0))
            seedBlit.endEncoding()

            // Clear offscreen, then render each group with .load to accumulate.
            let clearDesc = MTLRenderPassDescriptor()
            clearDesc.colorAttachments[0].texture = offscreen
            clearDesc.colorAttachments[0].loadAction = .clear
            clearDesc.colorAttachments[0].storeAction = .store
            clearDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
            if let clearEncoder = commandBuffer.makeRenderCommandEncoder(descriptor: clearDesc) {
                clearEncoder.endEncoding()
            }

            for (index, (group, data)) in sortedGroupData.enumerated() {
                // Per-group blur: use this group's frost level for its backdrop blur.
                let groupFrost = data.shapes.isEmpty ? Float(0)
                    : data.shapes.map(\.frost).reduce(0, +) / Float(data.shapes.count)
                let groupChildren = group.children
                let groupMinDim = groupChildren.map { CGFloat(min($0.frame.width, $0.frame.height)) }.reduce(0, +)
                    / max(CGFloat(groupChildren.count), 1)
                let groupBlurRadius = Holodeck.blurRadius(
                    forFrost: CGFloat(groupFrost),
                    nodeSize: CGSize(width: groupMinDim, height: groupMinDim)
                )

                let groupBlurred: MTLTexture
                if groupBlurRadius > 0, index > 0 {
                    groupBlurred = blurRenderer.blur(source: accTex, radius: groupBlurRadius, commandBuffer: commandBuffer) ?? accBlur
                } else {
                    groupBlurred = accBlur
                }

                // Render each group into offscreen with premultiplied blending (load existing).
                let passDesc = MTLRenderPassDescriptor()
                passDesc.colorAttachments[0].texture = offscreen
                passDesc.colorAttachments[0].loadAction = .load
                passDesc.colorAttachments[0].storeAction = .store
                guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDesc) else { continue }

                gooRenderer.encodeComposite(
                    blurredBackground: groupBlurred,
                    sharpBackground: accTex,
                    shapes: data.shapes,
                    uniforms: data.uniforms,
                    sdfTextures: data.sdfTextures,
                    groupCenter: .zero,
                    groupSize: groupSize,
                    viewportUniforms: HolodeckViewportUniforms(viewProjection: ortho),
                    encoder: encoder
                )
                encoder.endEncoding()

                // Composite this group's output onto the accumulation texture
                // so the next group sees it in its backdrop.
                if index < sortedGroupData.count - 1 {
                    gooRenderer.encodeOverComposite(
                        source: offscreen,
                        onto: accTex,
                        groupSize: groupSize,
                        commandBuffer: commandBuffer
                    )
                }
            }
        } else {
            // Single-pass: original path — all groups into one encoder.
            let passDesc = MTLRenderPassDescriptor()
            passDesc.colorAttachments[0].texture = offscreen
            passDesc.colorAttachments[0].loadAction = .clear
            passDesc.colorAttachments[0].storeAction = .store
            passDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
            guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDesc) else { return }

            for (_, data) in sortedGroupData {
                gooRenderer.encodeComposite(
                    blurredBackground: blurred,
                    sharpBackground: regionCopy,
                    shapes: data.shapes,
                    uniforms: data.uniforms,
                    sdfTextures: data.sdfTextures,
                    groupCenter: .zero,
                    groupSize: groupSize,
                    viewportUniforms: HolodeckViewportUniforms(viewProjection: ortho),
                    encoder: encoder
                )
            }

            encoder.endEncoding()
        }

        // Blit offscreen → drawable for display.
        guard let blit = commandBuffer.makeBlitCommandEncoder() else { return }
        blit.copy(
            from: offscreen, sourceSlice: 0, sourceLevel: 0,
            sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
            sourceSize: MTLSize(width: drawableW, height: drawableH, depth: 1),
            to: drawable.texture, destinationSlice: 0, destinationLevel: 0,
            destinationOrigin: MTLOrigin(x: 0, y: 0, z: 0)
        )
        blit.endEncoding()

        // Generate luminance mask for adaptive foreground when the backdrop changed,
        // or periodically for dynamic backdrops (~every 66ms at 60fps).
        let needsAdaptiveForeground = groups.flatMap(\.children).contains { $0.style.needsLuminanceMask }
            || style.needsLuminanceMask
        maskFrameCounter &+= 1
        let hasDynamicBackdropForMask = style.dynamicBackdrop
            || groups.flatMap(\.children).contains(where: \.style.dynamicBackdrop)
        let shouldRegenerateMask = backdropChanged
            || (hasDynamicBackdropForMask && maskFrameCounter % 4 == 0)
        if needsAdaptiveForeground && shouldRegenerateMask {
            let maskScale = Float(metalLayer.contentsScale)
            let viewSize = CGSize(width: CGFloat(gw), height: CGFloat(gh))
            let avgFrostForMask = allChildren.isEmpty ? Float(style.frost)
                : allChildren.map { Float($0.style.frost) }.reduce(0, +) / Float(allChildren.count)
            let maskTex = glassRenderer.encodeLuminanceMask(
                backgroundTexture: regionCopy,
                viewSize: viewSize,
                backingScale: maskScale,
                frost: avgFrostForMask,
                tintColor: premultipliedRGBA(style.tintColor),
                tintOpacity: Float(style.tintOpacity),
                commandBuffer: commandBuffer
            )
            // iOS uses .shared storage — no blit synchronization needed.

            // GPU-side AF probe: sample the mask for all children in one dispatch.
            var probeBuffer: MTLBuffer?
            var probeChildIDs: [String] = []
            if let maskTex, onAFProbeResults != nil {
                let scopeFrame = CGRect(origin: .zero, size: CGSize(width: CGFloat(gw), height: CGFloat(gh)))
                var elements: [(center: CGPoint, size: CGSize)] = []
                for child in allChildren {
                    probeChildIDs.append(child.id)
                    elements.append((
                        center: CGPoint(x: child.frame.midX, y: child.frame.midY),
                        size: CGSize(width: child.frame.width, height: child.frame.height)
                    ))
                }
                probeBuffer = glassRenderer.encodeAFProbe(
                    maskTexture: maskTex,
                    elements: elements,
                    glassFrame: scopeFrame,
                    commandBuffer: commandBuffer
                )
            }

            nonisolated(unsafe) let sendableTex = maskTex
            nonisolated(unsafe) let sendableProbeBuffer = probeBuffer
            let capturedChildIDs = probeChildIDs
            commandBuffer.addCompletedHandler { [weak self] _ in
                DispatchQueue.main.async {
                    self?.onLuminanceMaskUpdate?(sendableTex)
                    if let buf = sendableProbeBuffer {
                        let ptr = buf.contents().bindMemory(to: Float.self, capacity: capturedChildIDs.count)
                        var results: [String: CGFloat] = [:]
                        for (i, childID) in capturedChildIDs.enumerated() {
                            results[childID] = CGFloat(ptr[i])
                        }
                        self?.onAFProbeResults?(results)
                    }
                }
            }
        }

        commandBuffer.commit()
        commandBuffer.waitUntilScheduled()
        drawable.present()

        // Publish the offscreen texture to the shared store so standalone glass
        // views (which may be siblings, not children of the scope) can read it.
        // On Apple Silicon (unified memory), .shared textures are coherent —
        // getBytes returns the previous frame's completed content even if the
        // current frame's command buffer is still in flight.
        ConfluenceOutputStore.texture = offscreen
        ConfluenceOutputStore.renderCount &+= 1
        if let window {
            ConfluenceOutputStore.scopeFrame = convert(bounds, to: window)
        }
    }

    private func ensureOffscreenTexture(width: Int, height: Int) {
        guard (width, height) != offscreenSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        // .shared so standalone glass can read pixels on CPU for backdrop compositing.
        desc.storageMode = .shared

        offscreenTexture = metalLayer.device!.makeTexture(descriptor: desc)
        offscreenTexture?.label = "CrystalKit Goo Output"
        offscreenSize = (width, height)
    }

    private func updateLuminance(from texture: MTLTexture) {
        guard onLuminanceUpdate != nil || onBackdropColorUpdate != nil else { return }
        let tw = texture.width, th = texture.height
        guard tw > 0, th > 0 else { return }
        let bw = bounds.width, bh = bounds.height
        guard bw > 0, bh > 0 else { return }

        var pixel = [UInt8](repeating: 0, count: 4)
        let bytesPerRow = tw * 4

        for group in groups {
            for child in group.children {
                // Child frames are in scope-local coords; texture covers full scope.
                let pixelX = Int((child.frame.midX / bw) * CGFloat(tw))
                let pixelY = Int((child.frame.midY / bh) * CGFloat(th))
                let cx = max(0, min(pixelX, tw - 1))
                let cy = max(0, min(pixelY, th - 1))

                // Center probe for color — single point preserves backdrop saturation.
                // Averaging spread probes kills chroma when backdrop is multicolored.
                texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                                 from: MTLRegionMake2D(cx, cy, 1, 1), mipmapLevel: 0)
                let centerR = Float(pixel[2]) / 255, centerG = Float(pixel[1]) / 255, centerB = Float(pixel[0]) / 255
                let centerColor = SIMD3<Float>(centerR, centerG, centerB)

                // 5-probe average for luminance — spatial spread is fine for brightness.
                let sx = max(Int(child.frame.width / bw * CGFloat(tw) * 0.3), max(tw * 3 / 100, 1))
                let sy = max(Int(child.frame.height / bh * CGFloat(th) * 0.3), max(th * 3 / 100, 1))
                let probes = [
                    (cx, cy),
                    (min(cx + sx, tw - 1), cy), (max(cx - sx, 0), cy),
                    (cx, min(cy + sy, th - 1)), (cx, max(cy - sy, 0))
                ]

                var totalLum: Float = 0
                for (px, py) in probes {
                    texture.getBytes(&pixel, bytesPerRow: bytesPerRow,
                                     from: MTLRegionMake2D(px, py, 1, 1), mipmapLevel: 0)
                    let r = Float(pixel[2]) / 255, g = Float(pixel[1]) / 255, b = Float(pixel[0]) / 255
                    totalLum += 0.2126 * r + 0.7152 * g + 0.0722 * b
                }

                let avg = CGFloat(totalLum / 5)
                let prevLum = lastReportedLuminance[child.id] ?? 0.5
                if abs(avg - prevLum) > 0.05 {
                    lastReportedLuminance[child.id] = avg
                    onLuminanceUpdate?([child.id], avg)
                }

                let prevColor = lastReportedColor[child.id] ?? SIMD3(repeating: 0.5)
                if simd_length(centerColor - prevColor) > 0.05 {
                    lastReportedColor[child.id] = centerColor
                    onBackdropColorUpdate?([child.id], centerColor)
                }
            }
        }
    }

    private func ensureAccumulationTexture(width: Int, height: Int, device: MTLDevice) {
        guard (width, height) != accumulationSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        desc.storageMode = .private

        accumulationTexture = device.makeTexture(descriptor: desc)
        accumulationTexture?.label = "CrystalKit Accumulation Sharp"
        accumulationBlurred = device.makeTexture(descriptor: desc)
        accumulationBlurred?.label = "CrystalKit Accumulation Blurred"
        accumulationSize = (width, height)
    }

}

#endif
