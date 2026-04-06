// ShapeDescriptor.swift
// CrystalKit

import CoreGraphics
import Metal

/// Describes the shape that the glass effect is applied to.
///
/// The glass shader uses analytic SDFs for rectangles and ellipses,
/// and JFA-generated SDF textures for complex shapes (polygons, stars, paths).
///
/// **Simple API** (SwiftUI modifier): uses `.roundedRect`, `.capsule`, `.circle`.
/// **Advanced API** (direct renderer): uses `.polygon`, `.star`, or `.custom` with an SDF texture.
public enum ShapeDescriptor: @unchecked Sendable {

    /// Rounded rectangle with per-corner radii and optional smoothing.
    case roundedRect(
        cornerRadii: CornerRadii = .zero,
        smoothing: CGFloat = 0
    )

    /// Capsule (pill shape) — rendered as an ellipse.
    case capsule

    /// Circle — rendered as an ellipse.
    case circle

    /// Ellipse — same shader path as circle/capsule but semantically distinct.
    case ellipse

    /// Regular polygon (triangle, pentagon, hexagon, etc.).
    case polygon(
        sides: Int,
        cornerRadius: CGFloat = 0
    )

    /// Star shape with inner/outer radius control.
    case star(
        points: Int,
        innerRadius: CGFloat = 0.5,
        outerRadius: CGFloat = 1.0,
        cornerRadius: CGFloat = 0,
        innerCornerRadius: CGFloat = 0
    )

    /// Custom shape with a pre-computed SDF texture (for paths, booleans, etc.).
    /// The caller provides the JFA-generated SDF texture and its padding.
    case custom(sdfTexture: any MTLTexture, padding: CGFloat)

    /// Per-corner radii for rounded rectangles.
    public struct CornerRadii: Codable, Sendable, Equatable {
        public var topLeft: CGFloat
        public var topRight: CGFloat
        public var bottomRight: CGFloat
        public var bottomLeft: CGFloat

        public init(
            topLeft: CGFloat = 0,
            topRight: CGFloat = 0,
            bottomRight: CGFloat = 0,
            bottomLeft: CGFloat = 0
        ) {
            self.topLeft = topLeft
            self.topRight = topRight
            self.bottomRight = bottomRight
            self.bottomLeft = bottomLeft
        }

        /// Uniform radii (same value for all corners).
        public init(uniform radius: CGFloat) {
            self.topLeft = radius
            self.topRight = radius
            self.bottomRight = radius
            self.bottomLeft = radius
        }

        public static let zero = CornerRadii()

        /// SIMD4 for the shader: (TL, TR, BR, BL).
        var simd: SIMD4<Float> {
            SIMD4<Float>(
                Float(topLeft), Float(topRight),
                Float(bottomRight), Float(bottomLeft)
            )
        }
    }
}

// MARK: - Shader Mapping

extension ShapeDescriptor {

    /// The Metal shader `shapeType` uniform value.
    /// 0 = rectangle, 1 = ellipse, 2 = polygon, 3 = star.
    public var metalShapeType: UInt32 {
        switch self {
        case .roundedRect: 0
        case .capsule, .circle, .ellipse: 1
        case .polygon: 2
        case .star: 3
        case .custom: 0 // custom uses SDF texture, shapeType is irrelevant
        }
    }

    /// Corner radii as SIMD4 for the shader.
    var cornerRadiiSIMD: SIMD4<Float> {
        switch self {
        case .roundedRect(let radii, _):
            radii.simd
        default:
            .zero
        }
    }

    /// Corner smoothing factor for the shader (superellipse).
    var smoothing: CGFloat {
        switch self {
        case .roundedRect(_, let smoothing): smoothing
        default: 0
        }
    }

    /// Number of sides/points for polygon/star shapes.
    var sides: UInt32 {
        switch self {
        case .polygon(let sides, _): UInt32(max(sides, 3))
        case .star(let points, _, _, _, _): UInt32(max(points, 3))
        default: 5
        }
    }

    /// Star/polygon inner radius ratio.
    var innerRadius: Float {
        switch self {
        case .star(_, let inner, _, _, _): Float(max(min(inner, 1.0), 0.0))
        default: 0.5
        }
    }

    /// Star outer radius ratio.
    var outerRadius: Float {
        switch self {
        case .star(_, _, let outer, _, _): Float(max(min(outer, 1.0), 0.0))
        default: 1.0
        }
    }

    /// Polygon/star outer corner radius.
    var polygonBorderRadius: Float {
        switch self {
        case .polygon(_, let r): Float(max(r, 0))
        case .star(_, _, _, let r, _): Float(max(r, 0))
        default: 0
        }
    }

    /// Star inner corner radius.
    var starInnerBorderRadius: Float {
        switch self {
        case .star(_, _, _, _, let r): Float(max(r, 0))
        default: 0
        }
    }

    /// Whether the shader should use an SDF texture instead of analytic SDF.
    var useSDFTexture: Bool {
        if case .custom = self { return true }
        return false
    }

    /// The SDF texture, if this is a custom shape.
    var sdfTexture: (any MTLTexture)? {
        if case .custom(let tex, _) = self { return tex }
        return nil
    }

    /// SDF texture padding in normalized coordinates.
    var sdfPadding: CGFloat {
        if case .custom(_, let padding) = self { return padding }
        return 0
    }
}

// MARK: - Equatable

extension ShapeDescriptor: Equatable {
    public static func == (lhs: ShapeDescriptor, rhs: ShapeDescriptor) -> Bool {
        switch (lhs, rhs) {
        case (.roundedRect(let lr, let ls), .roundedRect(let rr, let rs)):
            return lr.topLeft == rr.topLeft && lr.topRight == rr.topRight
                && lr.bottomRight == rr.bottomRight && lr.bottomLeft == rr.bottomLeft
                && ls == rs
        case (.capsule, .capsule), (.circle, .circle), (.ellipse, .ellipse):
            return true
        case (.polygon(let ls, let lr), .polygon(let rs, let rr)):
            return ls == rs && lr == rr
        case (.star(let lp, let li, let lo, let lcr, let licr),
              .star(let rp, let ri, let ro, let rcr, let ricr)):
            return lp == rp && li == ri && lo == ro && lcr == rcr && licr == ricr
        case (.custom, .custom):
            return false // MTLTexture can't be compared; always treat as changed
        default:
            return false
        }
    }
}

// MARK: - Codable

extension ShapeDescriptor: Codable {

    private enum CodingKeys: String, CodingKey {
        case type, cornerRadii, smoothing, sides, cornerRadius
        case points, innerRadius, outerRadius, innerCornerRadius
    }

    private enum ShapeType: String, Codable {
        case roundedRect, capsule, circle, ellipse, polygon, star
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .roundedRect(let radii, let smoothing):
            try container.encode(ShapeType.roundedRect, forKey: .type)
            try container.encode(radii, forKey: .cornerRadii)
            try container.encode(smoothing, forKey: .smoothing)
        case .capsule:
            try container.encode(ShapeType.capsule, forKey: .type)
        case .circle:
            try container.encode(ShapeType.circle, forKey: .type)
        case .ellipse:
            try container.encode(ShapeType.ellipse, forKey: .type)
        case .polygon(let sides, let cornerRadius):
            try container.encode(ShapeType.polygon, forKey: .type)
            try container.encode(sides, forKey: .sides)
            try container.encode(cornerRadius, forKey: .cornerRadius)
        case .star(let points, let innerRadius, let outerRadius, let cornerRadius, let innerCornerRadius):
            try container.encode(ShapeType.star, forKey: .type)
            try container.encode(points, forKey: .points)
            try container.encode(innerRadius, forKey: .innerRadius)
            try container.encode(outerRadius, forKey: .outerRadius)
            try container.encode(cornerRadius, forKey: .cornerRadius)
            try container.encode(innerCornerRadius, forKey: .innerCornerRadius)
        case .custom:
            throw EncodingError.invalidValue(
                self,
                EncodingError.Context(
                    codingPath: encoder.codingPath,
                    debugDescription: "ShapeDescriptor.custom cannot be encoded — it holds a GPU texture resource."
                )
            )
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(ShapeType.self, forKey: .type)
        switch type {
        case .roundedRect:
            let radii = try container.decodeIfPresent(CornerRadii.self, forKey: .cornerRadii) ?? .zero
            let smoothing = try container.decodeIfPresent(CGFloat.self, forKey: .smoothing) ?? 0
            self = .roundedRect(cornerRadii: radii, smoothing: smoothing)
        case .capsule:
            self = .capsule
        case .circle:
            self = .circle
        case .ellipse:
            self = .ellipse
        case .polygon:
            let sides = try container.decode(Int.self, forKey: .sides)
            let cornerRadius = try container.decodeIfPresent(CGFloat.self, forKey: .cornerRadius) ?? 0
            self = .polygon(sides: sides, cornerRadius: cornerRadius)
        case .star:
            let points = try container.decode(Int.self, forKey: .points)
            let innerRadius = try container.decodeIfPresent(CGFloat.self, forKey: .innerRadius) ?? 0.5
            let outerRadius = try container.decodeIfPresent(CGFloat.self, forKey: .outerRadius) ?? 1.0
            let cornerRadius = try container.decodeIfPresent(CGFloat.self, forKey: .cornerRadius) ?? 0
            let innerCornerRadius = try container.decodeIfPresent(CGFloat.self, forKey: .innerCornerRadius) ?? 0
            self = .star(points: points, innerRadius: innerRadius, outerRadius: outerRadius,
                        cornerRadius: cornerRadius, innerCornerRadius: innerCornerRadius)
        }
    }
}

// MARK: - Convenience Initializers

extension ShapeDescriptor {

    /// Rounded rectangle with a uniform corner radius.
    public static func roundedRect(cornerRadius: CGFloat, smoothing: CGFloat = 0) -> ShapeDescriptor {
        .roundedRect(cornerRadii: .init(uniform: cornerRadius), smoothing: smoothing)
    }
}
