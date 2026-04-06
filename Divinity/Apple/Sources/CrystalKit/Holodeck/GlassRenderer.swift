// Holodeck.swift
// CrystalKit
//
// Composites blurred background with specular highlights, tint, and refraction
// to produce a Liquid Glass effect using pure Metal shaders.
// Adapted from Swiftlight's LiquidHolodeck — no external dependencies.

import Metal
import simd
import SwiftUI
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "Holodeck")

// MARK: - Uniforms

/// Uniforms for the glass composite fragment shader.
public struct HolodeckCompositeUniforms {
    var tintColor: SIMD4<Float>
    var tintOpacity: Float
    var desaturation: Float
    var specularIntensity: Float
    var glassVariant: UInt32
    var cornerRadii: SIMD4<Float>
    var size: SIMD2<Float>
    var blurPadding: SIMD2<Float>
    var refractionStrength: Float
    var dispersion: Float
    var depthScale: Float
    var splayStrength: Float
    var lightAngle: Float
    var lightIntensity: Float
    var lightBanding: Float
    var shapeType: UInt32
    var sides: UInt32
    var innerRadius: Float
    var outerRadius: Float
    var polygonBorderRadius: Float
    var starInnerBorderRadius: Float
    var cornerSmoothing: Float
    var maskConfig: SIMD4<Float>
    var edgeWidth: Float
    var frost: Float
    var rotation: Float
    var canvasZoom: Float
    var cursorWorldPos: SIMD2<Float>  // Cursor position in world/canvas coords. (-inf,-inf)=inactive.
    var cursorActive: Float           // 1.0 = cursor fluid mode, 0.0 = fixed angular mode
    var resonanceEnabled: UInt32      // 1 = adaptive bg-tinting, 0 = static tint only
    var viewportCenter: SIMD2<Float>  // Shape center in [0,1] viewport UV (for edge parallax)
    var viewportScale: Float          // Shape diagonal / viewport diagonal (0→1+, for parallax scaling)
    var smoothedResonanceTint: SIMD3<Float> = .zero  // CPU anchor tint for temporal smoothing
    var resonanceBlendFactor: Float = 0  // 0-1: how much fresh probe overrides anchor
    var _pad3: UInt32 = 0  // struct alignment padding
    var luminanceEnabled: UInt32 = 0           // 1 = inner light/shadow from backdrop luminance
    var brillianceCount: UInt32 = 0             // Number of active light sources (0-4)
    var brillianceSource0: SIMD2<Float> = .zero  // World position of light source 0
    var brillianceSource1: SIMD2<Float> = .zero  // World position of light source 1
    var brillianceSource2: SIMD2<Float> = .zero  // World position of light source 2
    var brillianceSource3: SIMD2<Float> = .zero  // World position of light source 3
    var brillianceMargin: Float = 0              // Quad expansion for flare bleed (canvas pts)
    var brillianceTint0: SIMD3<Float> = SIMD3<Float>(repeating: 1)  // Per-light adaptive tint (white if unused)
    var brillianceTint1: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    var brillianceTint2: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    var brillianceTint3: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    var cropUVOffset: SIMD2<Float> = .zero   // Top-left UV of glass region in full backdrop texture
    var cropUVScale: SIMD2<Float> = .one     // UV span of glass region in full backdrop texture
    var glowIntensity: Float = 0       // 0 = off, 0-1+ = glow strength
    var glowBlendMode: UInt32 = 0      // 0 = screen, 1 = additive, 2 = soft light
}

/// View-projection matrix for positioning the glass quad.
public struct HolodeckViewportUniforms {
    public var viewProjection: simd_float4x4

    public init(viewProjection: simd_float4x4) {
        self.viewProjection = viewProjection
    }
}

/// Vertex data for positioning a glass quad.
public struct HolodeckQuadInstance {
    var position: SIMD2<Float>
    var size: SIMD2<Float>
    var rotation: Float
    var opacity: Float
    var scale: Float
    var _pad: Float = 0
    var captureHalfExtent: SIMD2<Float> = .zero
    var captureOffset: SIMD2<Float> = .zero
}

// MARK: - Glass Renderer

@MainActor
public final class Holodeck {

    private let device: MTLDevice
    private let blurRenderer: FrostHolodeck
    private var compositePipelineState: MTLRenderPipelineState?
    private var rimLightPipelineState: MTLRenderPipelineState?

    // Rim light intermediate texture (rendered before composite, sampled with refraction)
    private var rimLightTexture: MTLTexture?
    private var rimLightTextureSize: SIMD2<Int> = .zero

    // Luminance mask for adaptive foreground
    private let maskFrostHolodeck: FrostHolodeck
    private var luminanceMaskPipelineState: MTLRenderPipelineState?
    public private(set) var luminanceMaskTexture: MTLTexture?
    private var luminanceMaskTextureSize: SIMD2<Int> = .zero

    // GPU-side AF probing (compute kernel)
    private var afProbePipeline: MTLComputePipelineState?
    private var afProbeInputBuffer: MTLBuffer?
    private var afProbeOutputBuffer: MTLBuffer?
    private var afProbeCapacity: Int = 0

    public init(device: MTLDevice, blurRenderer: FrostHolodeck) {
        self.device = device
        self.blurRenderer = blurRenderer
        self.maskFrostHolodeck = FrostHolodeck(device: device)
        setupPipeline()
    }

    public var isReady: Bool { compositePipelineState != nil && rimLightPipelineState != nil && luminanceMaskPipelineState != nil && blurRenderer.isReady }

    // MARK: - Pipeline Setup

    private func setupPipeline() {
        let library: MTLLibrary
        do {
            library = try ShaderLibraryCache.library(source: HolodeckShaderSource.source, cacheKey: HolodeckShaderSource.self, device: device)
        } catch {
            logger.error("Failed to compile glass composite shaders: \(error.localizedDescription)")
            return
        }

        guard let vertexFunction = library.makeFunction(name: "vertex_glass_quad"),
              let fragmentFunction = library.makeFunction(name: "fragment_glass_composite"),
              let rimLightFunction = library.makeFunction(name: "fragment_rim_light") else {
            logger.error("Failed to find glass shader functions")
            return
        }

        // Composite pipeline (premultiplied alpha blending)
        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = vertexFunction
        descriptor.fragmentFunction = fragmentFunction
        descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        let colorAttachment = descriptor.colorAttachments[0]!
        colorAttachment.isBlendingEnabled = true
        colorAttachment.rgbBlendOperation = .add
        colorAttachment.alphaBlendOperation = .add
        colorAttachment.sourceRGBBlendFactor = .one
        colorAttachment.destinationRGBBlendFactor = .oneMinusSourceAlpha
        colorAttachment.sourceAlphaBlendFactor = .one
        colorAttachment.destinationAlphaBlendFactor = .oneMinusSourceAlpha

        do {
            compositePipelineState = try device.makeRenderPipelineState(descriptor: descriptor)
            logger.info("CrystalKit glass composite pipeline initialized")
        } catch {
            logger.error("Failed to create glass composite pipeline: \(error)")
        }

        // Rim light pipeline (renders to intermediate texture, no blending needed)
        let rimDescriptor = MTLRenderPipelineDescriptor()
        rimDescriptor.vertexFunction = vertexFunction
        rimDescriptor.fragmentFunction = rimLightFunction
        rimDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        do {
            rimLightPipelineState = try device.makeRenderPipelineState(descriptor: rimDescriptor)
            logger.info("CrystalKit rim light pipeline initialized")
        } catch {
            logger.error("Failed to create rim light pipeline: \(error)")
        }

        // Luminance mask pipeline (for adaptive foreground)
        if let luminanceMaskFunction = library.makeFunction(name: "fragment_glass_luminance_mask") {
            let lumMaskDescriptor = MTLRenderPipelineDescriptor()
            lumMaskDescriptor.vertexFunction = vertexFunction
            lumMaskDescriptor.fragmentFunction = luminanceMaskFunction
            lumMaskDescriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

            do {
                luminanceMaskPipelineState = try device.makeRenderPipelineState(descriptor: lumMaskDescriptor)
                logger.info("CrystalKit luminance mask pipeline initialized")
            } catch {
                logger.error("Failed to create luminance mask pipeline: \(error)")
            }
        }

        // AF probe compute pipeline (GPU-side luminance sampling)
        if let probeKernel = library.makeFunction(name: "kernel_af_probe") {
            do {
                afProbePipeline = try device.makeComputePipelineState(function: probeKernel)
                logger.info("CrystalKit AF probe compute pipeline initialized")
            } catch {
                logger.error("Failed to create AF probe pipeline: \(error)")
            }
        }

    }

    // MARK: - Build Uniforms

    /// Builds glass uniforms from a `FacetStyle` and shape descriptor.
    ///
    /// This is the public API for both simple (SwiftUI modifier) and advanced (direct renderer) usage.
    /// The `ShapeDescriptor` provides all shape-specific parameters that the shader needs.
    public static func buildUniforms(
        style: FacetStyle,
        shape: ShapeDescriptor,
        size: CGSize,
        blurredTextureSize: SIMD2<Float>,
        blurPadding: SIMD2<Float>,
        maskPadding: SIMD2<Float> = .zero,
        rotation: Float = 0,
        canvasZoom: Float = 1.0,
        cursorWorldPos: SIMD2<Float> = SIMD2<Float>(-.infinity, -.infinity),
        viewportCenter: SIMD2<Float> = SIMD2<Float>(0.5, 0.5),
        viewportScale: Float = 0.2,
        smoothedResonanceTint: SIMD3<Float>? = nil,
        resonanceBlendFactor: Float = 0.08,
        smoothedBrillianceTints: [SIMD3<Float>] = [],
        cropUVOffset: SIMD2<Float> = .zero,
        cropUVScale: SIMD2<Float> = .one
    ) -> HolodeckCompositeUniforms {
        let tintColor = premultipliedRGBA(style.tintColor)

        let cornerRadii: SIMD4<Float>
        if let overrideRadius = style.cornerRadius {
            let radius = Float(max(0, overrideRadius))
            cornerRadii = SIMD4<Float>(repeating: radius)
        } else {
            cornerRadii = shape.cornerRadiiSIMD
        }

        // Normalized blur padding for UV remapping
        let padX = blurredTextureSize.x > 0
            ? min(max(blurPadding.x / blurredTextureSize.x, 0.0), 0.49)
            : Float(0)
        let padY = blurredTextureSize.y > 0
            ? min(max(blurPadding.y / blurredTextureSize.y, 0.0), 0.49)
            : Float(0)

        // Map 0-1 properties to shader-friendly ranges (identical to Swiftlight)
        let remapped = Float(style.refraction) * 0.75
        let refractionStr = powf(remapped, 2.0) * 0.06

        let splayStr = Float(style.splay) * 0.5
        let specIntensity = Float(style.lightIntensity)
        let depthScale: Float = 1.0 + Float(style.depth) * 0.3
        let dispersionStr = Float(style.dispersion) * 0.03
        let lightAngle = Float(style.lightRotation) * 2.0 * .pi

        return HolodeckCompositeUniforms(
            tintColor: tintColor,
            tintOpacity: Float(style.tintOpacity),
            desaturation: 0.0,
            specularIntensity: specIntensity,
            glassVariant: style.variant == .regular ? 0 : 1,
            cornerRadii: cornerRadii,
            size: SIMD2<Float>(Float(size.width), Float(size.height)),
            blurPadding: SIMD2<Float>(padX, padY),
            refractionStrength: refractionStr,
            dispersion: dispersionStr,
            depthScale: depthScale,
            splayStrength: splayStr,
            lightAngle: lightAngle,
            lightIntensity: specIntensity,
            lightBanding: Float(max(min(style.lightBanding, 1.0), 0.0)),
            shapeType: shape.metalShapeType,
            sides: shape.sides,
            innerRadius: shape.innerRadius,
            outerRadius: shape.outerRadius,
            polygonBorderRadius: shape.polygonBorderRadius,
            starInnerBorderRadius: shape.starInnerBorderRadius,
            cornerSmoothing: Float(shape.smoothing),
            maskConfig: SIMD4<Float>(maskPadding.x, maskPadding.y, shape.useSDFTexture ? 1 : 0, 0),
            edgeWidth: Float(max(min(style.edgeWidth, 1.0), -1.0)),
            frost: Float(max(min(style.frost, 1.0), 0.0)),
            rotation: rotation,
            canvasZoom: canvasZoom,
            cursorWorldPos: cursorWorldPos,
            cursorActive: cursorWorldPos.x.isFinite ? 1.0 : 0.0,
            resonanceEnabled: style.resonance ? 1 : 0,
            viewportCenter: viewportCenter,
            viewportScale: viewportScale,
            smoothedResonanceTint: smoothedResonanceTint ?? .zero,
            resonanceBlendFactor: smoothedResonanceTint != nil ? resonanceBlendFactor : 0.0,
            luminanceEnabled: style.luminance ? 1 : 0,
            brillianceCount: UInt32(min(style.brillianceSources.count, 4)),
            brillianceSource0: style.brillianceSources.count > 0 ? style.brillianceSources[0] : .zero,
            brillianceSource1: style.brillianceSources.count > 1 ? style.brillianceSources[1] : .zero,
            brillianceSource2: style.brillianceSources.count > 2 ? style.brillianceSources[2] : .zero,
            brillianceSource3: style.brillianceSources.count > 3 ? style.brillianceSources[3] : .zero,
            brillianceMargin: !style.brillianceSources.isEmpty ? Float(min(size.width, size.height)) * 0.5 : 0,
            brillianceTint0: smoothedBrillianceTints.count > 0 ? smoothedBrillianceTints[0] : SIMD3(repeating: 1),
            brillianceTint1: smoothedBrillianceTints.count > 1 ? smoothedBrillianceTints[1] : SIMD3(repeating: 1),
            brillianceTint2: smoothedBrillianceTints.count > 2 ? smoothedBrillianceTints[2] : SIMD3(repeating: 1),
            brillianceTint3: smoothedBrillianceTints.count > 3 ? smoothedBrillianceTints[3] : SIMD3(repeating: 1),
            cropUVOffset: cropUVOffset,
            cropUVScale: cropUVScale,
            glowIntensity: 0,
            glowBlendMode: 0
        )
    }

    /// Returns the blur radius for a given frost intensity.
    ///
    /// Exponential curve: each 10% increment roughly doubles the perceived frosting.
    /// - `frost 0.0` → 0px (perfectly clear)
    /// - `frost 0.5` → ~36px (gentle frost)
    /// - `frost 1.0` → ~260px (fully frosted)
    public static func blurRadius(forFrost frost: CGFloat) -> Float {
        let f = Float(max(min(frost, 1.0), 0.0))
        if f < 0.001 { return 0 }
        let k: Float = 4.0
        let maxRadius: Float = 260.0
        return maxRadius * (exp(k * f) - 1.0) / (exp(k) - 1.0)
    }

    /// Size-aware blur radius that scales proportionally to the node so frost
    /// looks visually consistent across different shape sizes. Anchored at a
    /// 200pt reference — shapes larger than 200pt get more blur, smaller get less.
    public static func blurRadius(forFrost frost: CGFloat, nodeSize: CGSize) -> Float {
        let base = blurRadius(forFrost: frost)
        guard base > 0 else { return 0 }
        let minDim = Float(min(nodeSize.width, nodeSize.height))
        let scale = max(minDim / 200.0, 0.1)
        return base * scale
    }

    // MARK: - Cursor / Gaze Light

    /// Converts a `CGPoint` cursor/gaze position to `SIMD2<Float>` for passing
    /// to `buildUniforms(cursorWorldPos:)`.
    ///
    /// The position should be in the same coordinate space as the element positions
    /// (canvas/world coordinates). The shader uses world-space distance from each
    /// pixel to this point, so all glass elements share the same light source.
    public static func cursorWorldPosition(_ point: CGPoint) -> SIMD2<Float> {
        SIMD2<Float>(Float(point.x), Float(point.y))
    }

    // MARK: - Rim Light Texture

    /// Ensures the rim light intermediate texture exists at the required size.
    private func ensureRimLightTexture(width: Int, height: Int) {
        let needed = SIMD2<Int>(width, height)
        if rimLightTextureSize == needed, rimLightTexture != nil { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: max(width, 1),
            height: max(height, 1),
            mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        desc.storageMode = .private
        rimLightTexture = device.makeTexture(descriptor: desc)
        rimLightTextureSize = needed
    }

    /// Renders the rim light to an intermediate texture.
    ///
    /// Call this before `encodeComposite` — the resulting texture is sampled
    /// with refraction UV offsets so the light bends with the glass.
    public func encodeRimLight(
        maskTexture: MTLTexture?,
        uniforms: HolodeckCompositeUniforms,
        viewportUniforms: HolodeckViewportUniforms,
        nodePosition: SIMD2<Float>,
        nodeSize: SIMD2<Float>,
        nodeRotation: Float,
        nodeOpacity: Float,
        nodeScale: Float,
        captureHalfExtent: SIMD2<Float>,
        captureOffset: SIMD2<Float> = .zero,
        backingScale: Float = 1.0,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        guard let pipeline = rimLightPipelineState else { return nil }

        // Compute the axis-aligned bounding box of the rotated quad so the
        // rim light texture is large enough to hold the full rotated shape.
        let cosR = abs(cos(nodeRotation))
        let sinR = abs(sin(nodeRotation))
        let aabbW = nodeSize.x * cosR + nodeSize.y * sinR
        let aabbH = nodeSize.x * sinR + nodeSize.y * cosR

        // Include backingScale so the rim light texture matches the Retina
        // drawable resolution. Without this, the rim texture is at 1× even
        // when the output drawable is at 2× or 3×, causing visible aliasing.
        let effectiveScale = nodeScale * backingScale
        let texW = Int(ceil(aabbW * effectiveScale))
        let texH = Int(ceil(aabbH * effectiveScale))
        ensureRimLightTexture(width: texW, height: texH)
        guard let rimTex = rimLightTexture else { return nil }

        let renderDesc = MTLRenderPassDescriptor()
        renderDesc.colorAttachments[0].texture = rimTex
        renderDesc.colorAttachments[0].loadAction = .clear
        renderDesc.colorAttachments[0].storeAction = .store
        renderDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderDesc) else { return nil }

        encoder.setRenderPipelineState(pipeline)

        // Ortho projection stays at nodeScale (world-space units) so the
        // shader's distance-based lighting math is unchanged. The larger
        // texture + viewport give us more pixels for the same world area.
        let scaledAabbW = aabbW * nodeScale
        let scaledAabbH = aabbH * nodeScale
        let rimOrtho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / scaledAabbW, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / scaledAabbH, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(-2.0 * nodePosition.x / scaledAabbW,
                          2.0 * nodePosition.y / scaledAabbH, 0, 1)
        ))

        var instance = HolodeckQuadInstance(
            position: nodePosition,
            size: nodeSize,
            rotation: nodeRotation,
            opacity: nodeOpacity,
            scale: nodeScale,
            captureHalfExtent: SIMD2<Float>(scaledAabbW * 0.5, scaledAabbH * 0.5),
            captureOffset: captureOffset
        )

        var vp = HolodeckViewportUniforms(viewProjection: rimOrtho)
        encoder.setVertexBytes(&vp, length: MemoryLayout<HolodeckViewportUniforms>.stride, index: 0)
        encoder.setVertexBytes(&instance, length: MemoryLayout<HolodeckQuadInstance>.stride, index: 1)

        // Rim light fragment uses: texture(0) = mask, buffer(0) = uniforms
        encoder.setFragmentTexture(maskTexture, index: 0)
        var glassUniforms = uniforms
        encoder.setFragmentBytes(&glassUniforms, length: MemoryLayout<HolodeckCompositeUniforms>.stride, index: 0)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
        encoder.endEncoding()

        return rimTex
    }

    // MARK: - Encode Composite

    /// Encodes the glass composite pass into an existing render command encoder.
    ///
    /// The caller is responsible for providing the blurred and sharp background textures
    /// (typically produced by `FrostHolodeck`), and the rim light texture (produced by
    /// `encodeRimLight`).
    public func encodeComposite(
        blurredBackground: MTLTexture,
        sharpBackground: MTLTexture,
        maskTexture: MTLTexture?,
        rimLightTexture: MTLTexture,
        glowTexture: MTLTexture? = nil,
        uniforms: HolodeckCompositeUniforms,
        viewportUniforms: HolodeckViewportUniforms,
        nodePosition: SIMD2<Float>,
        nodeSize: SIMD2<Float>,
        nodeRotation: Float,
        nodeOpacity: Float,
        nodeScale: Float,
        captureHalfExtent: SIMD2<Float>,
        captureOffset: SIMD2<Float> = .zero,
        encoder: MTLRenderCommandEncoder
    ) {
        guard let pipeline = compositePipelineState else { return }

        encoder.setRenderPipelineState(pipeline)

        let margin = uniforms.brillianceMargin
        var instance = HolodeckQuadInstance(
            position: nodePosition,
            size: nodeSize + SIMD2<Float>(margin * 2, margin * 2),
            rotation: nodeRotation,
            opacity: nodeOpacity,
            scale: nodeScale,
            captureHalfExtent: captureHalfExtent,
            captureOffset: captureOffset
        )

        var vp = viewportUniforms
        encoder.setVertexBytes(&vp, length: MemoryLayout<HolodeckViewportUniforms>.stride, index: 0)
        encoder.setVertexBytes(&instance, length: MemoryLayout<HolodeckQuadInstance>.stride, index: 1)

        encoder.setFragmentTexture(blurredBackground, index: 0)
        encoder.setFragmentTexture(sharpBackground, index: 1)
        encoder.setFragmentTexture(maskTexture ?? sharpBackground, index: 2)
        encoder.setFragmentTexture(rimLightTexture, index: 3)
        if let glow = glowTexture {
            encoder.setFragmentTexture(glow, index: 4)
        }
        var glassUniforms = uniforms
        encoder.setFragmentBytes(&glassUniforms, length: MemoryLayout<HolodeckCompositeUniforms>.stride, index: 0)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }

    // MARK: - Luminance Mask (Adaptive Foreground)

    /// Ensures the luminance mask texture exists at the required size.
    private func ensureLuminanceMaskTexture(width: Int, height: Int) {
        let needed = SIMD2<Int>(width, height)
        if luminanceMaskTextureSize == needed, luminanceMaskTexture != nil { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: max(width, 1),
            height: max(height, 1),
            mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        #if os(macOS)
        desc.storageMode = .managed
        #else
        desc.storageMode = .shared
        #endif
        luminanceMaskTexture = device.makeTexture(descriptor: desc)
        luminanceMaskTextureSize = needed
    }

    /// Renders a per-pixel luminance mask from the background texture.
    ///
    /// The mask covers only the glass region specified by `backdropUVRect`
    /// within the full backdrop. Zone AF children sample this at their own
    /// center (mapped to [0,1] within the glass frame) to pick white/black
    /// foreground independently per element.
    ///
    /// - Parameter backdropUVRect: The sub-region of the backdrop that corresponds
    ///   to this glass view, as (originX, originY, width, height) in [0,1] UV space.
    ///   Defaults to (0, 0, 1, 1) covering the full texture.
    public func encodeLuminanceMask(
        backgroundTexture: MTLTexture,
        viewSize: CGSize,
        backingScale: Float = 1.0,
        backdropUVRect: SIMD4<Float> = SIMD4<Float>(0, 0, 1, 1),
        frost: Float = 0.0,
        tintColor: SIMD4<Float> = .zero,
        tintOpacity: Float = 0.0,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        guard let pipeline = luminanceMaskPipelineState else { return nil }

        // Quarter-res: half each dimension = 4x fewer pixels to blur and store.
        // Zone AF only needs ~1 texel per element, and the frost-matched blur
        // destroys all high-frequency detail anyway.
        let maskScale = max(backingScale * 0.5, 1.0)
        let texW = max(Int(CGFloat(viewSize.width) * CGFloat(maskScale)), 1)
        let texH = max(Int(CGFloat(viewSize.height) * CGFloat(maskScale)), 1)
        ensureLuminanceMaskTexture(width: texW, height: texH)
        guard let maskTex = luminanceMaskTexture else { return nil }

        let renderDesc = MTLRenderPassDescriptor()
        renderDesc.colorAttachments[0].texture = maskTex
        renderDesc.colorAttachments[0].loadAction = .clear
        renderDesc.colorAttachments[0].storeAction = .store
        renderDesc.colorAttachments[0].clearColor = MTLClearColor(red: 0.5, green: 0.5, blue: 0.5, alpha: 1)

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderDesc) else { return nil }

        encoder.setRenderPipelineState(pipeline)

        // Fullscreen quad: identity ortho, centered at origin, size = view size
        let w = Float(viewSize.width)
        let h = Float(viewSize.height)
        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / w, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / h, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))

        var vp = HolodeckViewportUniforms(viewProjection: ortho)
        var instance = HolodeckQuadInstance(
            position: .zero,
            size: SIMD2<Float>(w, h),
            rotation: 0, opacity: 1, scale: 1,
            captureHalfExtent: SIMD2<Float>(w * 0.5, h * 0.5)
        )

        // Pass the UV sub-region so the shader samples only the glass area
        // from the full backdrop texture.
        var uvRect = backdropUVRect

        encoder.setVertexBytes(&vp, length: MemoryLayout<HolodeckViewportUniforms>.stride, index: 0)
        encoder.setVertexBytes(&instance, length: MemoryLayout<HolodeckQuadInstance>.stride, index: 1)
        encoder.setFragmentTexture(backgroundTexture, index: 0)
        encoder.setFragmentBytes(&uvRect, length: MemoryLayout<SIMD4<Float>>.stride, index: 0)

        // Tint compensation: un-premultiply to get linear RGB for the shader's mix().
        let a = max(tintColor.w, 1e-6)
        var tintParams = SIMD4<Float>(tintColor.x / a, tintColor.y / a, tintColor.z / a, tintOpacity)
        encoder.setFragmentBytes(&tintParams, length: MemoryLayout<SIMD4<Float>>.stride, index: 1)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
        encoder.endEncoding()

        // Blur the mask to match what the user sees through the frosted glass.
        // Use the same exponential curve as the visual frost blur so the mask's
        // spatial resolution matches the actual backdrop appearance. When frost is
        // zero (clear glass), the mask stays sharp — zone AF sees true boundaries.
        // Scale blur radius to match the downsampled mask resolution.
        // At half resolution, the same visual radius needs half as many pixels.
        let maskBlurRadius = Holodeck.blurRadius(forFrost: CGFloat(frost)) * maskScale / backingScale
        guard maskBlurRadius > 0, let blurredMask = maskFrostHolodeck.blur(
            source: maskTex,
            radius: maskBlurRadius,
            commandBuffer: commandBuffer
        ) else {
            return maskTex
        }

        // Copy blurred result back into the managed mask texture so it persists
        // across frames for the temporal feedback loop.
        if let blit = commandBuffer.makeBlitCommandEncoder() {
            blit.copy(
                from: blurredMask, sourceSlice: 0, sourceLevel: 0,
                sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
                sourceSize: MTLSize(width: texW, height: texH, depth: 1),
                to: maskTex, destinationSlice: 0, destinationLevel: 0,
                destinationOrigin: MTLOrigin(x: 0, y: 0, z: 0)
            )
            blit.endEncoding()
        }

        return maskTex
    }

    // MARK: - AF Probe (GPU-Side Luminance Sampling)

    /// Dispatches a compute shader that samples the luminance mask for all elements
    /// in one GPU pass, eliminating per-element CPU `getBytes` synchronization.
    ///
    /// Returns a `.shared` buffer of `Float` luminance values (one per element),
    /// readable on CPU after command buffer completion.
    ///
    /// - Parameters:
    ///   - maskTexture: The luminance mask texture (output of `encodeLuminanceMask`).
    ///   - elements: Per-element center and size in the glass frame's coordinate space.
    ///   - glassFrame: The glass surface's frame in global coordinates.
    ///   - commandBuffer: The command buffer to encode the dispatch into.
    /// - Returns: A buffer of `Float` luminance values, or `nil` if the pipeline is unavailable.
    public func encodeAFProbe(
        maskTexture: MTLTexture,
        elements: [(center: CGPoint, size: CGSize)],
        glassFrame: CGRect,
        commandBuffer: MTLCommandBuffer
    ) -> MTLBuffer? {
        guard let pipeline = afProbePipeline else { return nil }
        let count = elements.count
        guard count > 0 else { return nil }

        // Ensure buffers are large enough. SIMD4<Float> packs uv (xy) + uvSpan (zw).
        let inputSize = MemoryLayout<SIMD4<Float>>.stride * count
        let outputSize = MemoryLayout<Float>.stride * count

        if afProbeCapacity < count {
            afProbeInputBuffer = device.makeBuffer(length: inputSize, options: .storageModeShared)
            afProbeOutputBuffer = device.makeBuffer(length: outputSize, options: .storageModeShared)
            afProbeCapacity = count
        }

        guard let inputBuf = afProbeInputBuffer, let outputBuf = afProbeOutputBuffer else { return nil }

        // Fill input buffer with UV coordinates for each element.
        let inputPtr = inputBuf.contents().bindMemory(to: SIMD4<Float>.self, capacity: count)
        for (i, element) in elements.enumerated() {
            let u = Float((element.center.x - glassFrame.minX) / glassFrame.width)
            let v = Float((element.center.y - glassFrame.minY) / glassFrame.height)
            let du = Float(element.size.width * 0.25 / glassFrame.width)
            let dv = Float(element.size.height * 0.25 / glassFrame.height)
            inputPtr[i] = SIMD4<Float>(u, v, du, dv)
        }

        guard let encoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
        encoder.setComputePipelineState(pipeline)
        encoder.setTexture(maskTexture, index: 0)
        encoder.setBuffer(inputBuf, offset: 0, index: 0)
        encoder.setBuffer(outputBuf, offset: 0, index: 1)

        let threadgroupSize = min(pipeline.maxTotalThreadsPerThreadgroup, count)
        encoder.dispatchThreads(
            MTLSize(width: count, height: 1, depth: 1),
            threadsPerThreadgroup: MTLSize(width: threadgroupSize, height: 1, depth: 1)
        )
        encoder.endEncoding()

        return outputBuf
    }

}

// MARK: - Goo Structs

/// Per-shape data for one member of a goo group.
/// Memory layout must match the Metal `ConfluenceShapeDescriptor` struct exactly.
public struct ConfluenceShapeDescriptor {
    public var position: SIMD2<Float>
    public var halfSize: SIMD2<Float>
    public var cornerRadii: SIMD4<Float>
    public var rotation: Float
    public var shapeType: UInt32
    public var sides: UInt32
    public var innerRadius: Float
    public var outerRadius: Float
    public var polygonBorderRadius: Float
    public var starInnerBorderRadius: Float
    public var cornerSmoothing: Float
    public var sdfTextureIndex: Int32
    public var sdfMaskPadding: SIMD2<Float>
    public var sdfTexelToPoint: Float
    // Padding to align tintColor (float4) to 16-byte boundary, matching Metal struct layout
    private var _pad0: Float = 0
    private var _pad1: Float = 0
    private var _pad2: Float = 0
    // Per-shape material
    public var tintColor: SIMD4<Float> = .zero
    public var tintOpacity: Float = 0
    public var refractionStrength: Float = 0
    public var frost: Float = 0
    public var dispersion: Float = 0
    public var depthScale: Float = 1
    public var lightAngle: Float = 0
    public var lightIntensity: Float = 0.5
    public var lightBanding: Float = 0
    public var edgeWidth: Float = 0.5
    public var splayStrength: Float = 0

    public init(
        position: SIMD2<Float>, halfSize: SIMD2<Float>, cornerRadii: SIMD4<Float>,
        rotation: Float, shapeType: UInt32, sides: UInt32,
        innerRadius: Float = 0.5, outerRadius: Float = 1.0,
        polygonBorderRadius: Float = 0, starInnerBorderRadius: Float = 0,
        cornerSmoothing: Float = 0,
        sdfTextureIndex: Int32 = -1, sdfMaskPadding: SIMD2<Float> = .zero,
        sdfTexelToPoint: Float = 1.0
    ) {
        self.position = position; self.halfSize = halfSize; self.cornerRadii = cornerRadii
        self.rotation = rotation; self.shapeType = shapeType; self.sides = sides
        self.innerRadius = innerRadius; self.outerRadius = outerRadius
        self.polygonBorderRadius = polygonBorderRadius
        self.starInnerBorderRadius = starInnerBorderRadius
        self.cornerSmoothing = cornerSmoothing
        self.sdfTextureIndex = sdfTextureIndex; self.sdfMaskPadding = sdfMaskPadding
        self.sdfTexelToPoint = sdfTexelToPoint
    }
}

/// Shared uniforms for an entire goo group.
/// Memory layout must match the Metal `ConfluenceGroupUniforms` struct exactly.
public struct ConfluenceGroupUniforms {
    public var glassVariant: UInt32
    public var size: SIMD2<Float>
    public var splayStrength: Float
    public var canvasZoom: Float
    public var cursorWorldPos: SIMD2<Float>
    public var cursorActive: Float
    public var resonanceEnabled: UInt32
    public var shapeCount: UInt32
    public var smoothK: Float
    public var tiltY: Float = 0.0
    public var blurPadding: SIMD2<Float> = .zero
    public var captureHalfExtent: SIMD2<Float> = .zero
    public var captureOffset: SIMD2<Float> = .zero
    public var luminanceEnabled: UInt32 = 0
    public var brillianceCount: UInt32 = 0
    public var brillianceSource0: SIMD2<Float> = .zero
    public var brillianceSource1: SIMD2<Float> = .zero
    public var brillianceSource2: SIMD2<Float> = .zero
    public var brillianceSource3: SIMD2<Float> = .zero
    public var brillianceMargin: Float = 0
    public var brillianceTint0: SIMD3<Float> = SIMD3<Float>(repeating: 1)  // Per-light adaptive tint
    public var brillianceTint1: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    public var brillianceTint2: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    public var brillianceTint3: SIMD3<Float> = SIMD3<Float>(repeating: 1)
    public var glowIntensity: Float = 0       // 0 = off, 0-1+ = glow strength
    public var glowBlendMode: UInt32 = 0      // 0 = screen, 1 = additive, 2 = soft light
    public var appearanceMode: UInt32 = 0     // 0 = base (no contrast), 1 = light, 2 = dark
}

// MARK: - Goo Renderer

@MainActor
public final class ConfluenceHolodeck {

    private let device: MTLDevice
    private let blurRenderer: FrostHolodeck
    private var compositePipeline: MTLRenderPipelineState?
    private var passthroughPipeline: MTLRenderPipelineState?

    public var isReady: Bool { compositePipeline != nil }

    public init(device: MTLDevice, blurRenderer: FrostHolodeck) {
        self.device = device
        self.blurRenderer = blurRenderer
        setupPipelines()
    }

    private func setupPipelines() {
        let library: MTLLibrary
        do {
            library = try ShaderLibraryCache.library(source: HolodeckShaderSource.source, cacheKey: HolodeckShaderSource.self, device: device)
        } catch {
            logger.error("ConfluenceHolodeck: failed to compile shaders: \(error.localizedDescription)")
            return
        }

        guard let vert = library.makeFunction(name: "vertex_glass_quad"),
              let compFrag = library.makeFunction(name: "fragment_goo_composite") else {
            logger.error("ConfluenceHolodeck: missing shader functions")
            return
        }

        // Composite — premultiplied alpha blend into drawable
        let compDesc = MTLRenderPipelineDescriptor()
        compDesc.vertexFunction = vert
        compDesc.fragmentFunction = compFrag
        compDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
        let ca = compDesc.colorAttachments[0]!
        ca.isBlendingEnabled = true
        ca.rgbBlendOperation = .add
        ca.alphaBlendOperation = .add
        ca.sourceRGBBlendFactor = .one
        ca.destinationRGBBlendFactor = .oneMinusSourceAlpha
        ca.sourceAlphaBlendFactor = .one
        ca.destinationAlphaBlendFactor = .oneMinusSourceAlpha
        do {
            compositePipeline = try device.makeRenderPipelineState(descriptor: compDesc)
        } catch {
            logger.error("ConfluenceHolodeck: composite pipeline failed: \(error)")
        }

        // Passthrough — premultiplied alpha blend for over-compositing between passes
        if let ptFrag = library.makeFunction(name: "fragment_passthrough") {
            let ptDesc = MTLRenderPipelineDescriptor()
            ptDesc.vertexFunction = vert
            ptDesc.fragmentFunction = ptFrag
            ptDesc.colorAttachments[0].pixelFormat = .bgra8Unorm
            let ptCA = ptDesc.colorAttachments[0]!
            ptCA.isBlendingEnabled = true
            ptCA.rgbBlendOperation = .add
            ptCA.alphaBlendOperation = .add
            ptCA.sourceRGBBlendFactor = .one
            ptCA.destinationRGBBlendFactor = .oneMinusSourceAlpha
            ptCA.sourceAlphaBlendFactor = .one
            ptCA.destinationAlphaBlendFactor = .oneMinusSourceAlpha
            do {
                passthroughPipeline = try device.makeRenderPipelineState(descriptor: ptDesc)
            } catch {
                logger.error("ConfluenceHolodeck: passthrough pipeline failed: \(error)")
            }
        }
    }

    // MARK: - Composite Pass

    /// Encodes the goo composite pass into an existing render encoder.
    public func encodeComposite(
        blurredBackground: MTLTexture,
        sharpBackground: MTLTexture,
        shapes: [ConfluenceShapeDescriptor],
        uniforms: ConfluenceGroupUniforms,
        sdfTextures: [MTLTexture?],
        glowTexture: MTLTexture? = nil,
        groupCenter: SIMD2<Float>,
        groupSize: SIMD2<Float>,
        viewportUniforms: HolodeckViewportUniforms,
        encoder: MTLRenderCommandEncoder
    ) {
        guard let pipeline = compositePipeline else { return }
        encoder.setRenderPipelineState(pipeline)

        var vp = viewportUniforms
        encoder.setVertexBytes(&vp, length: MemoryLayout<HolodeckViewportUniforms>.stride, index: 0)

        let gooMargin = uniforms.brillianceMargin
        var inst = HolodeckQuadInstance(
            position: groupCenter,
            size: groupSize + SIMD2<Float>(gooMargin * 2, gooMargin * 2),
            rotation: 0, opacity: 1, scale: 1,
            captureHalfExtent: groupSize * 0.5,
            captureOffset: .zero
        )
        encoder.setVertexBytes(&inst, length: MemoryLayout<HolodeckQuadInstance>.stride, index: 1)

        // Texture slots: 0=blurred, 1=sharp, 2-9=SDF
        encoder.setFragmentTexture(blurredBackground, index: 0)
        encoder.setFragmentTexture(sharpBackground, index: 1)
        for i in 0..<min(sdfTextures.count, 8) {
            if let t = sdfTextures[i] { encoder.setFragmentTexture(t, index: 2 + i) }
        }
        if let glow = glowTexture {
            encoder.setFragmentTexture(glow, index: 10)
        }

        var gooUniforms = uniforms
        encoder.setFragmentBytes(&gooUniforms, length: MemoryLayout<ConfluenceGroupUniforms>.stride, index: 0)

        var shapesArray = shapes
        encoder.setFragmentBytes(
            &shapesArray,
            length: MemoryLayout<ConfluenceShapeDescriptor>.stride * max(shapes.count, 1),
            index: 1
        )

        // viewProjection at fragment buffer 2 — used to project worldPos to screen UV
        var vpMatrix = viewportUniforms.viewProjection
        encoder.setFragmentBytes(&vpMatrix, length: MemoryLayout<simd_float4x4>.stride, index: 2)

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }

    // MARK: - Over-Composite

    /// Composites a premultiplied-alpha source texture onto a destination texture
    /// using the "over" operator (src + dst * (1 - srcAlpha)).
    ///
    /// Used in multi-pass glass rendering to accumulate each group's output onto
    /// the accumulation texture before blurring for the next group.
    public func encodeOverComposite(
        source: MTLTexture,
        onto destination: MTLTexture,
        groupSize: SIMD2<Float>,
        commandBuffer: MTLCommandBuffer
    ) {
        guard let pipeline = passthroughPipeline else { return }

        let passDesc = MTLRenderPassDescriptor()
        passDesc.colorAttachments[0].texture = destination
        passDesc.colorAttachments[0].loadAction = .load  // preserve existing content
        passDesc.colorAttachments[0].storeAction = .store

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDesc) else { return }
        encoder.setRenderPipelineState(pipeline)

        // Identity ortho projection — fullscreen quad covering the entire texture.
        let gw = groupSize.x
        let gh = groupSize.y
        let ortho = simd_float4x4(columns: (
            SIMD4<Float>(2.0 / gw, 0, 0, 0),
            SIMD4<Float>(0, -2.0 / gh, 0, 0),
            SIMD4<Float>(0, 0, 1, 0),
            SIMD4<Float>(0, 0, 0, 1)
        ))

        var vp = HolodeckViewportUniforms(viewProjection: ortho)
        encoder.setVertexBytes(&vp, length: MemoryLayout<HolodeckViewportUniforms>.stride, index: 0)

        var inst = HolodeckQuadInstance(
            position: .zero,
            size: groupSize,
            rotation: 0, opacity: 1, scale: 1,
            captureHalfExtent: groupSize * 0.5,
            captureOffset: .zero
        )
        encoder.setVertexBytes(&inst, length: MemoryLayout<HolodeckQuadInstance>.stride, index: 1)

        encoder.setFragmentTexture(source, index: 0)
        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
        encoder.endEncoding()
    }

    // MARK: - Helpers

    public static func buildGroupUniforms(
        style: FacetStyle,
        groupSize: SIMD2<Float>,
        cursorWorldPos: SIMD2<Float> = SIMD2<Float>(-.infinity, -.infinity),
        shapeCount: UInt32,
        smoothK: Float = 40.0,
        smoothedBrillianceTints: [SIMD3<Float>] = []
    ) -> ConfluenceGroupUniforms {
        ConfluenceGroupUniforms(
            glassVariant: style.variant == .regular ? 0 : 1,
            size: groupSize,
            splayStrength: Float(style.splay) * 0.5,
            canvasZoom: 1.0,
            cursorWorldPos: cursorWorldPos,
            cursorActive: cursorWorldPos.x.isFinite ? 1.0 : 0.0,
            resonanceEnabled: style.resonance ? 1 : 0,
            shapeCount: shapeCount,
            smoothK: smoothK,
            luminanceEnabled: style.luminance ? 1 : 0,
            brillianceCount: UInt32(min(style.brillianceSources.count, 4)),
            brillianceSource0: style.brillianceSources.count > 0 ? style.brillianceSources[0] : .zero,
            brillianceSource1: style.brillianceSources.count > 1 ? style.brillianceSources[1] : .zero,
            brillianceSource2: style.brillianceSources.count > 2 ? style.brillianceSources[2] : .zero,
            brillianceSource3: style.brillianceSources.count > 3 ? style.brillianceSources[3] : .zero,
            brillianceMargin: !style.brillianceSources.isEmpty ? min(groupSize.x, groupSize.y) * 0.5 : 0,
            brillianceTint0: smoothedBrillianceTints.count > 0 ? smoothedBrillianceTints[0] : SIMD3(repeating: 1),
            brillianceTint1: smoothedBrillianceTints.count > 1 ? smoothedBrillianceTints[1] : SIMD3(repeating: 1),
            brillianceTint2: smoothedBrillianceTints.count > 2 ? smoothedBrillianceTints[2] : SIMD3(repeating: 1),
            brillianceTint3: smoothedBrillianceTints.count > 3 ? smoothedBrillianceTints[3] : SIMD3(repeating: 1),
            glowIntensity: 0,
            glowBlendMode: 0
        )
    }

    /// Fills per-shape material fields on a descriptor from a style.
    public static func applyMaterial(to desc: inout ConfluenceShapeDescriptor, style: FacetStyle) {
        desc.tintColor = premultipliedRGBA(style.tintColor)
        desc.tintOpacity = Float(style.tintOpacity)
        let remapped = Float(style.refraction) * 0.75
        desc.refractionStrength = powf(remapped, 2.0) * 0.06
        desc.frost = Float(max(min(style.frost, 1.0), 0.0))
        desc.dispersion = Float(style.dispersion) * 0.03
        desc.depthScale = 1.0 + Float(style.depth) * 0.3
        desc.lightAngle = Float(style.lightRotation) * 2.0 * .pi
        desc.lightIntensity = Float(style.lightIntensity)
        desc.lightBanding = Float(max(min(style.lightBanding, 1.0), 0.0))
        desc.edgeWidth = Float(max(min(style.edgeWidth, 1.0), -1.0))
        desc.splayStrength = Float(style.splay) * 0.5
    }

    /// Builds a ConfluenceShapeDescriptor from a ConfluenceChildInfo, offset by group center.
    /// `sdfTextureIndex` assigns the slot (0-7) for custom-path SDF textures, or -1 for analytic shapes.
    static func buildShapeDescriptor(
        child: ConfluenceChildInfo,
        groupCenter: CGPoint,
        style: FacetStyle,
        sdfTextureIndex: Int32 = -1
    ) -> ConfluenceShapeDescriptor {
        let relX = Float(child.frame.midX - groupCenter.x)
        let relY = Float(child.frame.midY - groupCenter.y)
        let hw = Float(child.frame.width * 0.5)
        let hh = Float(child.frame.height * 0.5)

        var shapeType: UInt32 = 0
        var sides: UInt32 = 0
        var innerRadius: Float = 0.5
        var outerRadius: Float = 1.0
        var polygonBorderRadius: Float = 0
        var starInnerBorderRadius: Float = 0
        var cornerSmoothing: Float = 0
        var cornerRadii = SIMD4<Float>(0, 0, 0, 0)
        var texIndex: Int32 = -1
        var maskPadding = SIMD2<Float>(0, 0)
        var texelToPoint: Float = 1.0

        switch child.shape {
        case .ellipse, .circle, .capsule:
            shapeType = 1
        case .polygon(let s, let cr):
            shapeType = 2; sides = UInt32(s); polygonBorderRadius = Float(cr)
        case .star(let pts, let ir, let or_, let cr, let icr):
            shapeType = 3; sides = UInt32(pts)
            innerRadius = Float(ir); outerRadius = Float(or_)
            polygonBorderRadius = Float(cr); starInnerBorderRadius = Float(icr)
        case .roundedRect(let radii, let smooth):
            shapeType = 0
            cornerRadii = SIMD4<Float>(
                Float(radii.topLeft), Float(radii.topRight),
                Float(radii.bottomRight), Float(radii.bottomLeft)
            )
            if let styleRadius = style.cornerRadius {
                let r = Float(max(0, styleRadius))
                cornerRadii = SIMD4<Float>(r, r, r, r)
            }
            cornerSmoothing = Float(smooth)
        case .custom(let sdfTex, let padding):
            shapeType = 0
            texIndex = sdfTextureIndex
            let pad = Float(padding)
            maskPadding = SIMD2<Float>(pad, pad)
            let rangeX = max(1.0 - 2.0 * pad, 1e-4)
            let rangeY = max(1.0 - 2.0 * pad, 1e-4)
            let texW = Float(sdfTex.width)
            let texH = Float(sdfTex.height)
            let shapeW = Float(child.frame.width)
            let shapeH = Float(child.frame.height)
            let texelsPerPtX = (texW * rangeX) / max(shapeW, 1)
            let texelsPerPtY = (texH * rangeY) / max(shapeH, 1)
            texelToPoint = 1.0 / max(0.5 * (texelsPerPtX + texelsPerPtY), 1e-4)
        }

        var desc = ConfluenceShapeDescriptor(
            position: SIMD2<Float>(relX, relY),
            halfSize: SIMD2<Float>(hw, hh),
            cornerRadii: cornerRadii,
            rotation: 0,
            shapeType: shapeType,
            sides: sides,
            innerRadius: innerRadius,
            outerRadius: outerRadius,
            polygonBorderRadius: polygonBorderRadius,
            starInnerBorderRadius: starInnerBorderRadius,
            cornerSmoothing: cornerSmoothing,
            sdfTextureIndex: texIndex,
            sdfMaskPadding: maskPadding,
            sdfTexelToPoint: texelToPoint
        )
        applyMaterial(to: &desc, style: child.style)
        return desc
    }
}
