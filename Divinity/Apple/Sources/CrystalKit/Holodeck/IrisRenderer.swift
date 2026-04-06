// IrisRenderer.swift
// CrystalKit
//
// Metal pipeline for the Iris thin-film interference material.
// Conforms to MaterialRenderer for use with the shared MaterialMetalView.

import Metal
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "IrisRenderer")

// MARK: - Dimple Descriptor

/// GPU-side dimple descriptor. 16 bytes each, passed via buffer(1).
/// Layout must match the Metal `IrisDimpleDescriptor` struct byte-for-byte.
struct IrisDimpleDescriptor {
    var position: SIMD2<Float>   //  0: 8 bytes
    var radius: Float            //  8: 4 bytes
    var depth: Float             // 12: 4 bytes
    // Total: 16 bytes (naturally aligned)
}

// MARK: - Uniforms

/// Uniforms for the Iris fragment shader.
/// Layout must match the Metal `IrisUniforms` struct byte-for-byte.
///
/// Alignment rules:
///   SIMD4<Float> (float4) = 16-byte alignment
///   SIMD2<Float> (float2) = 8-byte alignment
///   Float / UInt32         = 4-byte alignment
struct IrisUniforms {

    // ── Shape SDF (offset 0) ──
    var size: SIMD2<Float>                  //   0: 8 bytes
    var _alignPad0: SIMD2<Float> = .zero    //   8: 8 bytes (align cornerRadii to 16)
    var cornerRadii: SIMD4<Float>           //  16: 16 bytes
    var shapeType: UInt32                   //  32: 4 bytes
    var sides: UInt32                       //  36: 4 bytes
    var innerRadius: Float                  //  40: 4 bytes
    var outerRadius: Float                  //  44: 4 bytes
    var polygonBorderRadius: Float          //  48: 4 bytes
    var starInnerBorderRadius: Float        //  52: 4 bytes
    var cornerSmoothing: Float              //  56: 4 bytes
    var _shapePad: Float = 0               //  60: 4 bytes

    // ── Film Stack (offset 64) ──
    var layerCount: UInt32                  //  64: 4 bytes
    var baseThickness: Float               //  68: 4 bytes
    var thicknessSpread: Float             //  72: 4 bytes
    var thicknessScale: Float              //  76: 4 bytes

    // ── Dimple Count (offset 80) ──
    var dimpleCount: UInt32                 //  80: 4 bytes
    var _dimplePad: Float = 0              //  84: 4 bytes

    // ── Appearance (offset 88) ──
    var intensity: Float                   //  88: 4 bytes
    var brightness: Float                  //  92: 4 bytes
    var edgeFade: Float                    //  96: 4 bytes
    var refraction: Float                  // 100: 4 bytes
    var dispersion: Float                  // 104: 4 bytes
    var opacity: Float                     // 108: 4 bytes

    // ── Animation (offset 112) ──
    var time: Float                        // 112: 4 bytes
    var shiftSpeed: Float                  // 116: 4 bytes

    // ── Gleam Tracking (offset 120) ──
    var gleamPosition: SIMD2<Float>         // 120: 8 bytes (8-aligned ✓)
    var gleamInfluence: Float              // 128: 4 bytes
    var gleamRadius: Float                 // 132: 4 bytes

    // ── Backdrop UV Mapping (offset 136) ──
    var cropUVOffset: SIMD2<Float>         // 136: 8 bytes (8-aligned ✓)
    var cropUVScale: SIMD2<Float>          // 144: 8 bytes (8-aligned ✓)
    // Total: 152 bytes → stride 160 (16-byte aligned)
}

/// Viewport uniforms (identical layout to Holodeck).
struct IrisViewportUniforms {
    var viewProjection: simd_float4x4
}

/// Quad instance for positioning (identical layout to Holodeck).
struct IrisQuadInstance {
    var position: SIMD2<Float>
    var size: SIMD2<Float>
    var rotation: Float
    var opacity: Float
    var scale: Float
    var _pad: Float = 0
}

// MARK: - Renderer

@MainActor
public final class IrisRenderer: MaterialRenderer {

    private let device: MTLDevice
    private var pipelineState: MTLRenderPipelineState?

    public var isReady: Bool { pipelineState != nil }

    /// Current style — set by the modifier before each render.
    public var style: IrisStyle = IrisStyle()

    /// Elapsed time for animation — set by MaterialMetalView's onBeforeRender callback.
    public var time: Float = 0

    /// Gleam (cursor/hover/tilt) position in unit coords (0-1).
    /// Default (0.5, 0.5) = centered = no dimple shift.
    public var gleamPosition: SIMD2<Float> = SIMD2<Float>(0.5, 0.5)

    /// Backdrop crop UV offset — maps screenUV to full backdrop texture region.
    public var cropUVOffset: SIMD2<Float> = .zero

    /// Backdrop crop UV scale — maps screenUV to full backdrop texture region.
    public var cropUVScale: SIMD2<Float> = SIMD2<Float>(1, 1)

    public init(device: MTLDevice) {
        self.device = device
        setupPipeline()
    }

    private func setupPipeline() {
        let library: MTLLibrary
        do {
            library = try ShaderLibraryCache.library(
                source: IrisShaderSource.source,
                cacheKey: IrisShaderSource.self,
                device: device
            )
        } catch {
            logger.error("Failed to compile Iris shaders: \(error.localizedDescription)")
            return
        }

        guard let vertexFunc = library.makeFunction(name: "vertex_iris_quad"),
              let fragmentFunc = library.makeFunction(name: "fragment_iris") else {
            logger.error("Failed to find Iris shader functions")
            return
        }

        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = vertexFunc
        descriptor.fragmentFunction = fragmentFunc
        descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm

        // Premultiplied alpha blending (same as glass)
        let attachment = descriptor.colorAttachments[0]!
        attachment.isBlendingEnabled = true
        attachment.rgbBlendOperation = .add
        attachment.alphaBlendOperation = .add
        attachment.sourceRGBBlendFactor = .one
        attachment.destinationRGBBlendFactor = .oneMinusSourceAlpha
        attachment.sourceAlphaBlendFactor = .one
        attachment.destinationAlphaBlendFactor = .oneMinusSourceAlpha

        do {
            pipelineState = try device.makeRenderPipelineState(descriptor: descriptor)
            logger.info("CrystalKit Iris pipeline initialized")
        } catch {
            logger.error("Failed to create Iris pipeline: \(error.localizedDescription)")
        }
    }

    // MARK: - Build Uniforms

    static func buildUniforms(
        style: IrisStyle,
        shape: ShapeDescriptor,
        size: CGSize,
        time: Float,
        gleamPosition: SIMD2<Float> = SIMD2<Float>(0.5, 0.5),
        cropUVOffset: SIMD2<Float> = .zero,
        cropUVScale: SIMD2<Float> = SIMD2<Float>(1, 1)
    ) -> IrisUniforms {
        IrisUniforms(
            size: SIMD2<Float>(Float(size.width), Float(size.height)),
            cornerRadii: shape.cornerRadiiSIMD,
            shapeType: shape.metalShapeType,
            sides: shape.sides,
            innerRadius: shape.innerRadius,
            outerRadius: shape.outerRadius,
            polygonBorderRadius: shape.polygonBorderRadius,
            starInnerBorderRadius: shape.starInnerBorderRadius,
            cornerSmoothing: Float(shape.smoothing),
            layerCount: max(min(style.layerCount, 6), 1),
            baseThickness: Float(max(min(style.baseThickness, 3.0), 0.3)),
            thicknessSpread: Float(max(min(style.thicknessSpread, 1.0), 0.0)),
            thicknessScale: Float(max(min(style.thicknessScale, 3.0), 0.5)),
            dimpleCount: UInt32(min(style.dimples.count, irisDimpleMaxCount)),
            intensity: Float(max(min(style.intensity, 1.0), 0.0)),
            brightness: Float(max(min(style.brightness, 1.0), 0.0)),
            edgeFade: Float(max(min(style.edgeFade, 1.0), 0.0)),
            refraction: Float(max(min(style.refraction, 3.0), 0.0)),
            dispersion: Float(max(min(style.dispersion, 1.0), 0.0)),
            opacity: Float(max(min(style.opacity, 1.0), 0.0)),
            time: style.animated ? time : 0,
            shiftSpeed: Float(max(min(style.shiftSpeed, 3.0), 0.0)),
            gleamPosition: gleamPosition,
            gleamInfluence: Float(max(min(style.gleamInfluence, 1.0), 0.0)),
            gleamRadius: Float(max(min(style.gleamRadius, 0.5), 0.0)),
            cropUVOffset: cropUVOffset,
            cropUVScale: cropUVScale
        )
    }

    // MARK: - Build Dimple Descriptors

    /// Maps [IrisDimple] → [IrisDimpleDescriptor] with gleam offset applied.
    /// Caps at irisDimpleMaxCount.
    func buildDimpleDescriptors(
        gleamPosition: SIMD2<Float>,
        gleamInfluence: Float,
        gleamRadius: Float
    ) -> [IrisDimpleDescriptor] {
        let gleamOffset = (gleamPosition - SIMD2<Float>(0.5, 0.5))
            * gleamInfluence * gleamRadius
        let count = min(style.dimples.count, irisDimpleMaxCount)
        var descriptors: [IrisDimpleDescriptor] = []
        descriptors.reserveCapacity(count)
        for i in 0..<count {
            let dimple = style.dimples[i]
            let pos = SIMD2<Float>(
                Float(dimple.position.x),
                Float(dimple.position.y)
            ) + gleamOffset
            descriptors.append(IrisDimpleDescriptor(
                position: pos,
                radius: Float(max(min(dimple.radius, 1.0), 0.01)),
                depth: Float(max(min(dimple.depth, 1.0), -1.0))
            ))
        }
        return descriptors
    }

    // MARK: - MaterialRenderer

    public func encode(
        shape: ShapeDescriptor,
        size: SIMD2<Float>,
        viewProjection: simd_float4x4,
        encoder: MTLRenderCommandEncoder
    ) {
        encodeAt(
            shape: shape, size: size, position: .zero,
            rotation: 0, opacity: 1, scale: 1,
            viewProjection: viewProjection, encoder: encoder
        )
    }

    /// Encode at a specific canvas position — used by compositors that render
    /// multiple nodes onto a shared drawable.
    public func encodeAt(
        shape: ShapeDescriptor,
        size: SIMD2<Float>,
        position: SIMD2<Float>,
        rotation: Float,
        opacity: Float,
        scale: Float,
        viewProjection: simd_float4x4,
        backdropTexture: MTLTexture? = nil,
        encoder: MTLRenderCommandEncoder
    ) {
        guard let pipeline = pipelineState else { return }

        encoder.setRenderPipelineState(pipeline)

        var uniforms = Self.buildUniforms(
            style: style,
            shape: shape,
            size: CGSize(width: CGFloat(size.x), height: CGFloat(size.y)),
            time: time,
            gleamPosition: gleamPosition,
            cropUVOffset: cropUVOffset,
            cropUVScale: cropUVScale
        )

        var instance = IrisQuadInstance(
            position: position,
            size: size,
            rotation: rotation,
            opacity: opacity,
            scale: scale
        )

        var vp = IrisViewportUniforms(viewProjection: viewProjection)
        encoder.setVertexBytes(&vp, length: MemoryLayout<IrisViewportUniforms>.stride, index: 0)
        encoder.setVertexBytes(&instance, length: MemoryLayout<IrisQuadInstance>.stride, index: 1)

        // SDF texture for custom shapes
        if let maskTexture = shape.sdfTexture {
            encoder.setFragmentTexture(maskTexture, index: 0)
        }
        // Backdrop texture for refraction
        if let backdrop = backdropTexture {
            encoder.setFragmentTexture(backdrop, index: 1)
        }

        encoder.setFragmentBytes(&uniforms, length: MemoryLayout<IrisUniforms>.stride, index: 0)

        // Dimple array via buffer(1) — variable length, Confluence pattern
        var dimpleDescs = buildDimpleDescriptors(
            gleamPosition: gleamPosition,
            gleamInfluence: Float(style.gleamInfluence),
            gleamRadius: Float(style.gleamRadius)
        )
        if !dimpleDescs.isEmpty {
            encoder.setFragmentBytes(
                &dimpleDescs,
                length: MemoryLayout<IrisDimpleDescriptor>.stride * dimpleDescs.count,
                index: 1
            )
        }

        encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6)
    }
}
