// ShaderLibraryCache.swift
// CrystalKit
//
// Caches compiled MTLLibrary instances by source type so multiple renderers
// sharing the same shader source don't re-compile it from scratch.

import Metal

@MainActor
enum ShaderLibraryCache {

    /// Keyed by the `ObjectIdentifier` of the type that owns the source string
    /// (e.g. `HolodeckShaderSource.self`, `FrostHolodeck.self`).
    private static var cache: [ObjectIdentifier: MTLLibrary] = [:]

    /// Returns a compiled `MTLLibrary`, compiling only on the first call per source type.
    ///
    /// - Parameters:
    ///   - source: The Metal shader source string.
    ///   - cacheKey: The type that owns the source (used as the cache key).
    ///   - device: The Metal device to compile against.
    /// - Returns: A compiled `MTLLibrary`.
    static func library(
        source: String,
        cacheKey: Any.Type,
        device: MTLDevice
    ) throws -> MTLLibrary {
        let key = ObjectIdentifier(cacheKey)
        if let lib = cache[key] { return lib }
        let lib = try device.makeLibrary(source: source, options: nil)
        cache[key] = lib
        return lib
    }
}
