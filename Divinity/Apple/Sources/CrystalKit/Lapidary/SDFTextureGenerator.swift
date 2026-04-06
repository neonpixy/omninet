// SDFTextureGenerator.swift
// CrystalKit
//
// Generates signed distance field (SDF) textures from binary masks using the
// Jump Flood Algorithm (JFA). The SDF texture provides smooth distance values
// and gradients for the Crystal Glass shader, replacing the less accurate
// finite-difference-on-alpha approach for polygon, star, path, and boolean shapes.

import Metal
import simd
import os

private let logger = Logger(subsystem: "com.crystalkit", category: "SDFTextureGenerator")

// MARK: - SDF Texture Generator

@MainActor
public final class SDFTextureGenerator {

    private let device: MTLDevice
    private let seedPipeline: MTLComputePipelineState
    private let floodPipeline: MTLComputePipelineState
    private let distancePipeline: MTLComputePipelineState
    private let jacobiPipeline: MTLComputePipelineState
    private let composePipeline: MTLComputePipelineState

    /// Ping-pong textures for JFA (.rg16Uint — stores 2D nearest-boundary coordinates).
    /// Lazily allocated and resized as needed.
    private var pingTexture: MTLTexture?
    private var pongTexture: MTLTexture?
    private var currentSize: SIMD2<Int> = .zero

    public var isReady: Bool { true }

    public init?(device: MTLDevice) {
        self.device = device

        let library: MTLLibrary
        do {
            library = try device.makeLibrary(source: Self.shaderSource, options: nil)
        } catch {
            logger.error("Failed to compile SDF generator shaders: \(error.localizedDescription)")
            return nil
        }

        guard let seedFn = library.makeFunction(name: "jfa_seed"),
              let floodFn = library.makeFunction(name: "jfa_flood"),
              let distFn = library.makeFunction(name: "jfa_distance"),
              let jacobiFn = library.makeFunction(name: "heat_jacobi"),
              let composeFn = library.makeFunction(name: "heat_compose") else {
            logger.error("Failed to find SDF generator kernel functions")
            return nil
        }

        do {
            seedPipeline = try device.makeComputePipelineState(function: seedFn)
            floodPipeline = try device.makeComputePipelineState(function: floodFn)
            distancePipeline = try device.makeComputePipelineState(function: distFn)
            jacobiPipeline = try device.makeComputePipelineState(function: jacobiFn)
            composePipeline = try device.makeComputePipelineState(function: composeFn)
        } catch {
            logger.error("Failed to create SDF compute pipeline states: \(error)")
            return nil
        }

        logger.info("SDFTextureGenerator initialized")
    }

    // MARK: - Texture Management

    /// Ensures ping-pong textures are allocated at the required dimensions.
    private func ensureTextures(width: Int, height: Int) {
        let size = SIMD2<Int>(width, height)
        guard size != currentSize else { return }

        let desc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rg16Uint,
            width: width,
            height: height,
            mipmapped: false
        )
        desc.usage = [.shaderRead, .shaderWrite]
        desc.storageMode = .private

        pingTexture = device.makeTexture(descriptor: desc)
        pongTexture = device.makeTexture(descriptor: desc)
        currentSize = size

        pingTexture?.label = "JFA Ping"
        pongTexture?.label = "JFA Pong"
    }

    // MARK: - SDF Generation

    /// Generates an SDF texture with a smooth height field from a rasterized mask.
    ///
    /// The returned `.rgba16Float` texture stores:
    /// - `.r` = signed distance (negative inside, positive outside, zero on boundary) in texels
    /// - `.g` = unused (0)
    /// - `.b` = unused (0)
    /// - `.a` = smooth height field (0 at boundary, rising toward interior center)
    ///
    /// The height field is computed by solving the heat equation (Laplace's equation)
    /// via Jacobi iteration. It produces a smooth dome shape with no medial axis
    /// seams — the gradient transitions smoothly between edges everywhere.
    /// The fragment shader computes refraction direction from the height gradient.
    ///
    /// - Parameters:
    ///   - maskTexture: An RGBA texture whose alpha channel defines the shape boundary.
    ///   - commandBuffer: The command buffer to encode compute passes into.
    /// - Returns: An `.rgba16Float` SDF+height texture, or nil if generation fails.
    public func generateSDF(
        from maskTexture: MTLTexture,
        commandBuffer: MTLCommandBuffer
    ) -> MTLTexture? {
        let width = maskTexture.width
        let height = maskTexture.height
        guard width > 0, height > 0 else { return nil }

        ensureTextures(width: width, height: height)
        guard let ping = pingTexture, let pong = pongTexture else { return nil }

        // Create the output SDF texture (.rgba16Float).
        // Stores signed distance + inward direction for nearest-edge refraction.
        // Needs .shaderWrite for the jfa_distance compute kernel, and
        // .shaderRead for the glass fragment shader that samples it.
        let sdfDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .rgba16Float,
            width: width,
            height: height,
            mipmapped: false
        )
        sdfDesc.usage = [.shaderRead, .shaderWrite]
        sdfDesc.storageMode = .private

        guard let sdfTexture = device.makeTexture(descriptor: sdfDesc) else { return nil }
        sdfTexture.label = "SDF Output"

        // --- Pass 1: Seed ---
        // Identify boundary pixels from alpha transitions in the mask texture.
        guard let seedEncoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
        seedEncoder.label = "JFA Seed"
        seedEncoder.setComputePipelineState(seedPipeline)
        seedEncoder.setTexture(maskTexture, index: 0)
        seedEncoder.setTexture(ping, index: 1)
        dispatchThreads(encoder: seedEncoder, pipeline: seedPipeline, width: width, height: height)
        seedEncoder.endEncoding()

        // --- Pass 2: Flood ---
        // Propagate nearest boundary coordinates in log2(max(W,H)) passes.
        let maxDim = max(width, height)
        let passCount = Int(ceil(log2(Double(maxDim))))
        var stepSize = maxDim / 2
        var readTexture = ping
        var writeTexture = pong

        for pass in 0..<passCount {
            guard let floodEncoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
            floodEncoder.label = "JFA Flood \(pass)"
            floodEncoder.setComputePipelineState(floodPipeline)
            floodEncoder.setTexture(readTexture, index: 0)
            floodEncoder.setTexture(writeTexture, index: 1)
            var step = Int32(max(stepSize, 1))
            floodEncoder.setBytes(&step, length: MemoryLayout<Int32>.size, index: 0)
            dispatchThreads(encoder: floodEncoder, pipeline: floodPipeline, width: width, height: height)
            floodEncoder.endEncoding()

            // Ping-pong
            swap(&readTexture, &writeTexture)
            stepSize = max(stepSize / 2, 1)
        }

        // After all flood passes, readTexture contains the final Voronoi result.

        // --- Pass 3: Distance extraction ---
        // Convert nearest-boundary coordinates to signed distance values.
        guard let distEncoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
        distEncoder.label = "JFA Distance"
        distEncoder.setComputePipelineState(distancePipeline)
        distEncoder.setTexture(maskTexture, index: 0)
        distEncoder.setTexture(readTexture, index: 1)
        distEncoder.setTexture(sdfTexture, index: 2)
        dispatchThreads(encoder: distEncoder, pipeline: distancePipeline, width: width, height: height)
        distEncoder.endEncoding()

        // --- Pass 4: Heat equation (Jacobi iteration) ---
        // Solves ∇²h = 0 inside the shape with h = 0 on the boundary.
        // This produces the smoothest possible dome — no medial axis seams.
        // Seeded with the SDF distance (clamped to interior) so convergence
        // is fast. Each iteration: h[i,j] = avg(4 neighbors), boundary pinned to 0.
        //
        // We use two .r16Float ping-pong textures for the Jacobi iteration,
        // then compose the final height into the SDF texture's .a channel.

        let heatDesc = MTLTextureDescriptor.texture2DDescriptor(
            pixelFormat: .r16Float,
            width: width,
            height: height,
            mipmapped: false
        )
        heatDesc.usage = [.shaderRead, .shaderWrite]
        heatDesc.storageMode = .private

        guard let heatA = device.makeTexture(descriptor: heatDesc),
              let heatB = device.makeTexture(descriptor: heatDesc) else { return nil }
        heatA.label = "Heat A"
        heatB.label = "Heat B"

        // Jacobi iterations scale with texture size. The solver smooths
        // medial axis ridges over ~sqrt(iterations) texels. SDF-seeded
        // initialization means we only need to fix the skeleton, not
        // converge from scratch. Base: 64 for ≤512px, scaling up for
        // larger shapes so the height field stays seam-free.
        let jacobiIterations = maxDim <= 512 ? 64 : min(Int(64.0 * Float(maxDim) / 512.0), 256)

        var heatRead = heatA
        var heatWrite = heatB

        for i in 0..<jacobiIterations {
            guard let jacobiEncoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
            jacobiEncoder.label = "Heat Jacobi \(i)"
            jacobiEncoder.setComputePipelineState(jacobiPipeline)
            jacobiEncoder.setTexture(maskTexture, index: 0)  // boundary mask
            jacobiEncoder.setTexture(sdfTexture, index: 1)   // SDF (for seed on iter 0)
            jacobiEncoder.setTexture(heatRead, index: 2)     // previous iteration
            jacobiEncoder.setTexture(heatWrite, index: 3)    // output
            var iter = Int32(i)
            jacobiEncoder.setBytes(&iter, length: MemoryLayout<Int32>.size, index: 0)
            dispatchThreads(encoder: jacobiEncoder, pipeline: jacobiPipeline, width: width, height: height)
            jacobiEncoder.endEncoding()

            swap(&heatRead, &heatWrite)
        }

        // --- Pass 5: Compose SDF distance + height into final texture ---
        guard let finalTexture = device.makeTexture(descriptor: sdfDesc) else { return nil }
        finalTexture.label = "SDF + Height"

        guard let composeEncoder = commandBuffer.makeComputeCommandEncoder() else { return nil }
        composeEncoder.label = "Heat Compose"
        composeEncoder.setComputePipelineState(composePipeline)
        composeEncoder.setTexture(sdfTexture, index: 0)   // read SDF (.r)
        composeEncoder.setTexture(heatRead, index: 1)      // read height
        composeEncoder.setTexture(finalTexture, index: 2)  // write combined
        dispatchThreads(encoder: composeEncoder, pipeline: composePipeline, width: width, height: height)
        composeEncoder.endEncoding()

        return finalTexture
    }

    // MARK: - Dispatch Helpers

    private func dispatchThreads(
        encoder: MTLComputeCommandEncoder,
        pipeline: MTLComputePipelineState,
        width: Int,
        height: Int
    ) {
        let threadGroupWidth = min(pipeline.threadExecutionWidth, width)
        let threadGroupHeight = min(
            pipeline.maxTotalThreadsPerThreadgroup / pipeline.threadExecutionWidth,
            height
        )
        let threadGroupSize = MTLSize(width: threadGroupWidth, height: threadGroupHeight, depth: 1)
        let threadGroups = MTLSize(
            width: (width + threadGroupWidth - 1) / threadGroupWidth,
            height: (height + threadGroupHeight - 1) / threadGroupHeight,
            depth: 1
        )
        encoder.dispatchThreadgroups(threadGroups, threadsPerThreadgroup: threadGroupSize)
    }

    // MARK: - Metal Shader Source

    static let shaderSource = """
    #include <metal_stdlib>
    using namespace metal;

    // Sentinel value: "no nearest boundary pixel found yet."
    // 65535 is safe because Metal textures are limited to 16384 in any dimension.
    #define SENTINEL_X 65535
    #define SENTINEL_Y 65535

    // ─── Seed Pass ───────────────────────────────────────────────────────
    // Reads the alpha channel of the mask texture. Pixels on the boundary
    // (where alpha transitions across 0.5 among the 4-connected neighbors)
    // are seeded with their own coordinate. All other pixels get SENTINEL.

    kernel void jfa_seed(
        texture2d<float, access::read> mask [[texture(0)]],
        texture2d<ushort, access::write> output [[texture(1)]],
        uint2 gid [[thread_position_in_grid]]
    ) {
        uint w = mask.get_width();
        uint h = mask.get_height();
        if (gid.x >= w || gid.y >= h) return;

        float alpha = mask.read(gid).a;
        bool inside = alpha > 0.5;

        // Check 4-connected neighbors for a boundary transition.
        bool isBoundary = false;
        if (gid.x > 0) {
            isBoundary = isBoundary || ((mask.read(uint2(gid.x - 1, gid.y)).a > 0.5) != inside);
        }
        if (gid.x + 1 < w) {
            isBoundary = isBoundary || ((mask.read(uint2(gid.x + 1, gid.y)).a > 0.5) != inside);
        }
        if (gid.y > 0) {
            isBoundary = isBoundary || ((mask.read(uint2(gid.x, gid.y - 1)).a > 0.5) != inside);
        }
        if (gid.y + 1 < h) {
            isBoundary = isBoundary || ((mask.read(uint2(gid.x, gid.y + 1)).a > 0.5) != inside);
        }

        ushort2 coord = ushort2(gid.x, gid.y);
        output.write(isBoundary ? ushort4(coord.x, coord.y, 0, 0) : ushort4(SENTINEL_X, SENTINEL_Y, 0, 0), gid);
    }

    // ─── Flood Pass ──────────────────────────────────────────────────────
    // Standard JFA: each pixel checks 9 neighbors (self + 8 at current step
    // size) and keeps whichever neighbor's stored coordinate is closest.
    // Runs ceil(log2(max(W,H))) times with halving step sizes.

    kernel void jfa_flood(
        texture2d<ushort, access::read> input [[texture(0)]],
        texture2d<ushort, access::write> output [[texture(1)]],
        constant int &stepSize [[buffer(0)]],
        uint2 gid [[thread_position_in_grid]]
    ) {
        uint w = input.get_width();
        uint h = input.get_height();
        if (gid.x >= w || gid.y >= h) return;

        float bestDist = 1e10;
        ushort2 bestCoord = ushort2(SENTINEL_X, SENTINEL_Y);

        for (int dy = -1; dy <= 1; dy++) {
            for (int dx = -1; dx <= 1; dx++) {
                int2 neighbor = int2(gid) + int2(dx, dy) * stepSize;
                if (neighbor.x < 0 || neighbor.x >= int(w) ||
                    neighbor.y < 0 || neighbor.y >= int(h)) continue;

                ushort2 coord = input.read(uint2(neighbor)).rg;
                if (coord.x == SENTINEL_X && coord.y == SENTINEL_Y) continue;

                float dist = distance(float2(gid), float2(coord));
                if (dist < bestDist) {
                    bestDist = dist;
                    bestCoord = coord;
                }
            }
        }

        output.write(ushort4(bestCoord.x, bestCoord.y, 0, 0), gid);
    }

    // ─── Distance + Direction Extraction ────────────────────────────────
    // Converts the Voronoi result (nearest boundary coordinate per pixel)
    // into a signed distance field plus inward direction vector.
    //
    //   .r = signed distance (negative inside, positive outside) in texels
    //   .g = inward direction X (from nearest edge toward interior)
    //   .b = inward direction Y

    kernel void jfa_distance(
        texture2d<float, access::read> mask [[texture(0)]],
        texture2d<ushort, access::read> voronoi [[texture(1)]],
        texture2d<float, access::write> sdf [[texture(2)]],
        uint2 gid [[thread_position_in_grid]]
    ) {
        uint w = mask.get_width();
        uint h = mask.get_height();
        if (gid.x >= w || gid.y >= h) return;

        ushort2 nearest = voronoi.read(gid).rg;
        float dist;
        float2 inwardDir = float2(0.0);
        if (nearest.x == SENTINEL_X && nearest.y == SENTINEL_Y) {
            dist = float(max(w, h));
            sdf.write(float4(dist, 0.0, 0.0, 0.0), gid);
            return;
        } else {
            float2 pixelF = float2(gid);
            float2 nearestF = float2(nearest);
            dist = distance(pixelF, nearestF);

            if (dist > 0.5) {
                inwardDir = (pixelF - nearestF) / dist;
            }
        }

        bool inside = mask.read(gid).a > 0.5;
        float signedDist = inside ? -dist : dist;
        if (!inside) inwardDir = -inwardDir;

        sdf.write(float4(signedDist, 0.0, 0.0, 0.0), gid);
    }

    // ─── Heat Equation: Jacobi Iteration ─────────────────────────────
    // Solves ∇²h = 0 (Laplace's equation) inside the shape.
    // Boundary condition: h = 0 at the shape edge.
    // Initial condition: h = |SDF distance| (interior only) for fast convergence.
    //
    // The result is the smoothest possible dome — the gradient points smoothly
    // toward the nearest edge with no medial axis seams.

    kernel void heat_jacobi(
        texture2d<float, access::read> mask [[texture(0)]],
        texture2d<float, access::read> sdf [[texture(1)]],
        texture2d<float, access::read> prev [[texture(2)]],
        texture2d<float, access::write> next [[texture(3)]],
        constant int &iteration [[buffer(0)]],
        uint2 gid [[thread_position_in_grid]]
    ) {
        uint w = mask.get_width();
        uint h = mask.get_height();
        if (gid.x >= w || gid.y >= h) return;

        float alpha = mask.read(gid).a;

        if (alpha <= 0.5) {
            next.write(float4(0.0), gid);
            return;
        }

        bool nearBoundary = false;
        if (gid.x > 0)     nearBoundary = nearBoundary || (mask.read(uint2(gid.x - 1, gid.y)).a <= 0.5);
        if (gid.x + 1 < w) nearBoundary = nearBoundary || (mask.read(uint2(gid.x + 1, gid.y)).a <= 0.5);
        if (gid.y > 0)     nearBoundary = nearBoundary || (mask.read(uint2(gid.x, gid.y - 1)).a <= 0.5);
        if (gid.y + 1 < h) nearBoundary = nearBoundary || (mask.read(uint2(gid.x, gid.y + 1)).a <= 0.5);

        if (nearBoundary) {
            next.write(float4(0.0), gid);
            return;
        }

        if (iteration == 0) {
            float dist = abs(sdf.read(gid).r);
            next.write(float4(dist, 0.0, 0.0, 0.0), gid);
            return;
        }

        float left  = (gid.x > 0)     ? prev.read(uint2(gid.x - 1, gid.y)).r : 0.0;
        float right = (gid.x + 1 < w) ? prev.read(uint2(gid.x + 1, gid.y)).r : 0.0;
        float top   = (gid.y > 0)     ? prev.read(uint2(gid.x, gid.y - 1)).r : 0.0;
        float bot   = (gid.y + 1 < h) ? prev.read(uint2(gid.x, gid.y + 1)).r : 0.0;

        float avg = (left + right + top + bot) * 0.25;
        next.write(float4(avg, 0.0, 0.0, 0.0), gid);
    }

    // ─── Compose: merge height field into SDF texture ────────────────
    // Copies .r from SDF (signed distance) and .a from heat field (height).

    kernel void heat_compose(
        texture2d<float, access::read> sdf [[texture(0)]],
        texture2d<float, access::read> heat [[texture(1)]],
        texture2d<float, access::write> output [[texture(2)]],
        uint2 gid [[thread_position_in_grid]]
    ) {
        uint w = sdf.get_width();
        uint h = sdf.get_height();
        if (gid.x >= w || gid.y >= h) return;

        float signedDist = sdf.read(gid).r;
        float height = heat.read(gid).r;
        output.write(float4(signedDist, 0.0, 0.0, height), gid);
    }
    """
}
