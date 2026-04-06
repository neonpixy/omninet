// FrostHolodeck.swift
// CrystalKit
//
// Kawase blur pipeline: multi-pass blur approximation using ping-pong textures.
// Each pass samples 5 offset points (center + 4 diagonals), approximating
// a Gaussian blur at a fraction of the cost. 4-5 passes produce a visually
// convincing blur for shadows, background blur, and Liquid Glass.

import Metal
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "FrostHolodeck")

// MARK: - Kawase Blur Uniforms

/// Per-pass uniforms for the Kawase blur shader.
struct KawaseBlurUniforms {
    var texelSize: SIMD2<Float>   // 1.0 / textureSize
    var offset: Float             // Per-pass offset (increases each pass)
    var _pad: Float = 0
}

// MARK: - Blur Renderer

@MainActor
public final class FrostHolodeck {

    private let device: MTLDevice
    private var blurPipelineState: MTLRenderPipelineState?

    /// Ping-pong textures for multi-pass blur.
    /// Allocated on-demand and resized as needed.
    private var pingTexture: MTLTexture?
    private var pongTexture: MTLTexture?
    private var currentSize: SIMD2<Int> = .zero

    /// Cached region copy texture — reused across frames to avoid per-frame allocation.
    private var regionCopyTexture: MTLTexture?
    private var regionCopySize: (Int, Int) = (0, 0)

    public init(device: MTLDevice) {
        self.device = device
        setupPipeline()
    }

    public var isReady: Bool { blurPipelineState != nil }

    // MARK: - Pipeline Setup

    private func setupPipeline() {
        let library: MTLLibrary
        do {
            library = try ShaderLibraryCache.library(source: Self.shaderSource, cacheKey: FrostHolodeck.self, device: device)
        } catch {
            logger.error("Failed to compile Kawase blur shaders: \(error.localizedDescription)")
            return
        }

        guard let vertexFunction = library.makeFunction(name: "vertex_fullscreen_quad"),
              let fragmentFunction = library.makeFunction(name: "fragment_kawase_blur") else {
            logger.error("Failed to find Kawase blur shader functions")
            return
        }

        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = vertexFunction
        descriptor.fragmentFunction = fragmentFunction
        descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        // No blending — we're writing directly to the ping-pong texture
        descriptor.colorAttachments[0].isBlendingEnabled = false

        do {
            blurPipelineState = try device.makeRenderPipelineState(descriptor: descriptor)
            logger.info("Kawase blur pipeline initialized")
        } catch {
            logger.error("Failed to create Kawase blur pipeline state: \(error)")
        }
    }

    // MARK: - Texture Management

    /// Ensures ping-pong textures are allocated at the required size.
    private func ensureTextures(width: Int, height: Int) {
        let size = SIMD2<Int>(width, height)
        guard size != currentSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .bgra8Unorm,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.renderTarget, .shaderRead]
        desc.storageMode = .private

        pingTexture = device.makeTexture(descriptor: desc)
        pongTexture = device.makeTexture(descriptor: desc)
        currentSize = size

        pingTexture?.label = "CrystalKit Blur Ping"
        pongTexture?.label = "CrystalKit Blur Pong"
    }

    // MARK: - Blur Execution

    /// Applies Kawase blur to the source texture and returns the blurred result.
    ///
    /// - Parameters:
    ///   - source: The input texture to blur.
    ///   - radius: The desired blur radius in pixels. Determines number of passes.
    ///   - commandBuffer: The Metal command buffer to encode into.
    /// - Returns: The blurred texture (either ping or pong, depending on pass count).
    public func blur(
        source: MTLTexture,
        radius: Float,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        guard let blurPipelineState else { return nil }

        let width = source.width
        let height = source.height
        ensureTextures(width: width, height: height)

        guard let ping = pingTexture, let pong = pongTexture else { return nil }

        // Determine pass offsets based on blur radius.
        // More passes = larger blur. Each pass increases the sampling offset.
        let offsets = passOffsets(for: radius)
        if offsets.isEmpty { return source } // No blur needed

        let texelSize = SIMD2<Float>(1.0 / Float(width), 1.0 / Float(height))

        var readTexture = source
        var writeTexture = ping

        let passDescriptor = MTLRenderPassDescriptor()
        passDescriptor.colorAttachments[0].loadAction = .dontCare
        passDescriptor.colorAttachments[0].storeAction = .store

        for (index, offset) in offsets.enumerated() {
            // Alternate between ping and pong for writing
            writeTexture = (index % 2 == 0) ? ping : pong

            passDescriptor.colorAttachments[0].texture = writeTexture

            guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDescriptor) else {
                continue
            }

            encoder.setRenderPipelineState(blurPipelineState)
            encoder.setFragmentTexture(readTexture, index: 0)

            var uniforms = KawaseBlurUniforms(
                texelSize: texelSize,
                offset: offset
            )
            encoder.setFragmentBytes(&uniforms, length: MemoryLayout<KawaseBlurUniforms>.stride, index: 0)

            // Fullscreen triangle (3 vertices covering the screen)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
            encoder.endEncoding()

            readTexture = writeTexture
        }

        return writeTexture
    }

    /// Copies a rectangular region from a source texture to a cached texture.
    /// Used to capture the area behind a blur/glass view.
    /// The destination texture is reused across frames — only reallocated when dimensions change.
    public func copyRegion(
        from source: MTLTexture,
        region: MTLRegion,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        let w = region.size.width
        let h = region.size.height

        if regionCopySize != (w, h) || regionCopyTexture == nil {
            let desc = MTLTextureDescriptor.texture2DDescriptor(
                pixelFormat: source.pixelFormat,
                width: w,
                height: h,
                mipmapped: false
            )
            desc.usage = [.shaderRead, .renderTarget]
            desc.storageMode = .private

            guard let tex = device.makeTexture(descriptor: desc) else { return nil }
            tex.label = "CrystalKit BlurRegionCopy"
            regionCopyTexture = tex
            regionCopySize = (w, h)
        }

        guard let regionTexture = regionCopyTexture else { return nil }
        guard let blitEncoder = commandBuffer.makeBlitCommandEncoder() else { return nil }
        blitEncoder.copy(
            from: source,
            sourceSlice: 0,
            sourceLevel: 0,
            sourceOrigin: region.origin,
            sourceSize: region.size,
            to: regionTexture,
            destinationSlice: 0,
            destinationLevel: 0,
            destinationOrigin: MTLOrigin(x: 0, y: 0, z: 0)
        )
        blitEncoder.endEncoding()

        return regionTexture
    }

    /// Copies a source texture into a larger texture with padding on all sides.
    /// The source is centered; padding pixels are left black (the subsequent blur
    /// pass will smear edge content into the padding zone).
    public func copyWithPadding(
        from source: MTLTexture,
        padding: Int,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        let paddedW = source.width + padding * 2
        let paddedH = source.height + padding * 2
        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: source.pixelFormat,
            width: paddedW, height: paddedH, mipmapped: false
        )
        desc.usage = [.shaderRead, .renderTarget]
        desc.storageMode = .private

        guard let paddedTexture = device.makeTexture(descriptor: desc) else { return nil }
        paddedTexture.label = "CrystalKit PaddedCapture"

        guard let blitEncoder = commandBuffer.makeBlitCommandEncoder() else { return nil }
        blitEncoder.copy(
            from: source, sourceSlice: 0, sourceLevel: 0,
            sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
            sourceSize: MTLSize(width: source.width, height: source.height, depth: 1),
            to: paddedTexture, destinationSlice: 0, destinationLevel: 0,
            destinationOrigin: MTLOrigin(x: padding, y: padding, z: 0)
        )
        blitEncoder.endEncoding()

        return paddedTexture
    }

    // MARK: - Pass Configuration

    /// Returns Kawase pass offsets for a given blur radius.
    /// More passes with increasing offsets approximate larger Gaussian blurs.
    /// Static arrays avoid per-frame heap allocation.
    private static let offsets0: [Float] = []
    private static let offsets1: [Float] = [0, 1]
    private static let offsets2: [Float] = [0, 1, 2]
    private static let offsets3: [Float] = [0, 1, 2, 2]
    private static let offsets4: [Float] = [0, 1, 2, 2, 3]
    private static let offsets5: [Float] = [0, 1, 2, 3, 4, 4]
    private static let offsets6: [Float] = [0, 1, 2, 3, 4, 4, 5]
    private static let offsets7: [Float] = [0, 1, 2, 3, 4, 5, 6, 7]
    private static let offsets8: [Float] = [0, 1, 2, 3, 4, 5, 6, 7, 8]
    private static let offsets9: [Float] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    private static let offsets10: [Float] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    private static let offsets11: [Float] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]

    private func passOffsets(for radius: Float) -> [Float] {
        if radius < 1 { return Self.offsets0 }
        if radius < 4 { return Self.offsets1 }
        if radius < 8 { return Self.offsets2 }
        if radius < 16 { return Self.offsets3 }
        if radius < 32 { return Self.offsets4 }
        if radius < 48 { return Self.offsets5 }
        if radius < 70 { return Self.offsets6 }
        if radius < 90 { return Self.offsets7 }
        if radius < 150 { return Self.offsets8 }
        if radius < 250 { return Self.offsets9 }
        if radius < 350 { return Self.offsets10 }
        return Self.offsets11
    }

    // MARK: - Shader Source

    static let shaderSource = """
    #include <metal_stdlib>
    using namespace metal;

    struct KawaseBlurUniforms {
        float2 texelSize;
        float offset;
        float _pad;
    };

    struct FullscreenVertexOut {
        float4 position [[position]];
        float2 uv;
    };

    // Fullscreen triangle: 3 vertices that cover the entire screen.
    // More efficient than a fullscreen quad (6 vertices).
    vertex FullscreenVertexOut vertex_fullscreen_quad(uint vid [[vertex_id]]) {
        FullscreenVertexOut out;
        // Generate a triangle that covers [-1, 1] x [-1, 1]
        out.uv = float2((vid << 1) & 2, vid & 2);
        out.position = float4(out.uv * 2.0 - 1.0, 0.0, 1.0);
        // Flip Y for Metal (top-left origin)
        out.uv.y = 1.0 - out.uv.y;
        return out;
    }

    // Kawase blur pass: sample center + 4 diagonal offsets.
    // Each pass uses a larger offset, building up the blur progressively.
    fragment float4 fragment_kawase_blur(
        FullscreenVertexOut in [[stage_in]],
        texture2d<float> tex [[texture(0)]],
        constant KawaseBlurUniforms &uniforms [[buffer(0)]]
    ) {
        constexpr sampler s(mag_filter::linear, min_filter::linear, address::clamp_to_edge);

        float2 uv = in.uv;
        float2 off = (uniforms.offset + 0.5) * uniforms.texelSize;

        float4 sum = tex.sample(s, uv);
        sum += tex.sample(s, uv + float2( off.x,  off.y));
        sum += tex.sample(s, uv + float2(-off.x,  off.y));
        sum += tex.sample(s, uv + float2( off.x, -off.y));
        sum += tex.sample(s, uv + float2(-off.x, -off.y));

        return sum * 0.2; // Average of 5 samples
    }
    """
}
