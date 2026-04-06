// MaterialRenderer.swift
// CrystalKit
//
// Protocol for pluggable material renderers. Each material (glass, iris, etc.)
// implements this protocol. The shared MaterialMetalView delegates rendering
// to whichever renderer it's been given.

import Metal
import simd

/// Contract for a CrystalKit material renderer.
///
/// Conforming types own their Metal pipeline state and know how to encode
/// a single render pass for their material effect. The shared ``MaterialMetalView``
/// handles everything else (CAMetalLayer, display link, command buffer lifecycle).
///
/// To add a new material to CrystalKit:
/// 1. Create a renderer conforming to `MaterialRenderer`
/// 2. Create a style struct for configuration
/// 3. Create a SwiftUI modifier that wires them together via `MaterialMetalView`
@MainActor
public protocol MaterialRenderer: AnyObject {

    /// Whether the Metal pipeline compiled successfully and the renderer is usable.
    var isReady: Bool { get }

    /// Encode the material's render pass into the given command encoder.
    ///
    /// The encoder already has a render pass descriptor targeting the drawable texture
    /// with a clear load action. The renderer should set its pipeline state, bind
    /// textures/uniforms, and issue draw calls.
    ///
    /// - Parameters:
    ///   - shape: The shape to clip the material to (SDF evaluation).
    ///   - size: View size in points.
    ///   - viewProjection: Orthographic projection matrix (y-down, origin at center).
    ///   - encoder: The active render command encoder.
    func encode(
        shape: ShapeDescriptor,
        size: SIMD2<Float>,
        viewProjection: simd_float4x4,
        encoder: MTLRenderCommandEncoder
    )
}
