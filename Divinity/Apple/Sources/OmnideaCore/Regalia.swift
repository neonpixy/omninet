import COmnideaFFI
import Foundation

// MARK: - Codable types matching Rust Regalia JSON

/// Color matching Regalia's Ember. Serializes as hex string "#RRGGBB" or "#RRGGBBAA".
public struct Ember: Codable, Sendable, Hashable {
    public let red: Double
    public let green: Double
    public let blue: Double
    public let alpha: Double

    public init(red: Double, green: Double, blue: Double, alpha: Double = 1.0) {
        self.red = red
        self.green = green
        self.blue = blue
        self.alpha = alpha
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let hex = try container.decode(String.self)
        let trimmed = hex.hasPrefix("#") ? String(hex.dropFirst()) : hex
        guard trimmed.count == 6 || trimmed.count == 8 else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid hex color: \(hex)")
        }
        let scanner = Scanner(string: trimmed)
        var value: UInt64 = 0
        scanner.scanHexInt64(&value)
        if trimmed.count == 6 {
            red = Double((value >> 16) & 0xFF) / 255.0
            green = Double((value >> 8) & 0xFF) / 255.0
            blue = Double(value & 0xFF) / 255.0
            alpha = 1.0
        } else {
            red = Double((value >> 24) & 0xFF) / 255.0
            green = Double((value >> 16) & 0xFF) / 255.0
            blue = Double((value >> 8) & 0xFF) / 255.0
            alpha = Double(value & 0xFF) / 255.0
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        let r = UInt8((red * 255).rounded())
        let g = UInt8((green * 255).rounded())
        let b = UInt8((blue * 255).rounded())
        if abs(alpha - 1.0) < 0.001 {
            try container.encode(String(format: "#%02X%02X%02X", r, g, b))
        } else {
            let a = UInt8((alpha * 255).rounded())
            try container.encode(String(format: "#%02X%02X%02X%02X", r, g, b, a))
        }
    }
}

/// Semantic color palette matching Regalia's Crest.
public struct Crest: Codable, Sendable {
    public let primary: Ember
    public let secondary: Ember
    public let accent: Ember
    public let background: Ember
    public let surface: Ember
    public let onPrimary: Ember
    public let onBackground: Ember
    public let danger: Ember
    public let success: Ember
    public let warning: Ember
    public let info: Ember

    enum CodingKeys: String, CodingKey {
        case primary, secondary, accent, background, surface
        case onPrimary = "on_primary"
        case onBackground = "on_background"
        case danger, success, warning, info
    }
}

/// Appearance mode matching Regalia's Aspect.
public struct Aspect: Codable, Sendable {
    public let name: String

    public init(name: String) { self.name = name }

    public static let light = Aspect(name: "light")
    public static let dark = Aspect(name: "dark")
}

/// Complete theme matching Regalia's Reign.
public struct Reign: Codable, Sendable {
    public let name: String
    public let aspect: Aspect
}

/// Resolved layout node matching Regalia's Appointment.
public struct Appointment: Codable, Sendable {
    public let id: String
    public let x: Double
    public let y: Double
    public let width: Double
    public let height: Double
    public let contentX: Double
    public let contentY: Double
    public let contentWidth: Double
    public let contentHeight: Double
    public let sanctumId: String
    public let compositeZOrder: Double

    enum CodingKeys: String, CodingKey {
        case id, x, y, width, height
        case contentX = "content_x"
        case contentY = "content_y"
        case contentWidth = "content_width"
        case contentHeight = "content_height"
        case sanctumId = "sanctum_id"
        case compositeZOrder = "composite_z_order"
    }
}

/// Sanctum bounds: [x, y, width, height].
public struct SanctumBounds: Sendable {
    public let x: Double
    public let y: Double
    public let width: Double
    public let height: Double

    public init(_ array: [Double]) {
        x = array.count > 0 ? array[0] : 0
        y = array.count > 1 ? array[1] : 0
        width = array.count > 2 ? array[2] : 0
        height = array.count > 3 ? array[3] : 0
    }
}

/// Complete layout result matching Regalia's Domain.
public struct Domain: Codable, Sendable {
    public let appointments: [Appointment]
    public let sanctumBounds: [String: [Double]]
    public let bounds: [Double]

    enum CodingKeys: String, CodingKey {
        case appointments
        case sanctumBounds = "sanctum_bounds"
        case bounds
    }

    /// Get bounds for a named sanctum.
    public func boundsFor(_ sanctumId: String) -> SanctumBounds? {
        guard let arr = sanctumBounds[sanctumId] else { return nil }
        return SanctumBounds(arr)
    }
}

// MARK: - FFI bridge

/// Regalia FFI bridge — calls into the Rust core.
public enum RegaliaFFI {

    /// Get the default Reign theme.
    public static func defaultReign() throws -> Reign {
        guard let ptr = divi_regalia_default_reign() else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get default reign")
        }
        defer { divi_free_string(ptr) }
        let json = String(cString: ptr)
        return try JSONDecoder().decode(Reign.self, from: Data(json.utf8))
    }

    /// Resolve the Crest color palette for an aspect.
    public static func resolveCrest(aspect: String = "light") throws -> Crest {
        guard let ptr = divi_regalia_resolve_crest(nil, aspect) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to resolve crest")
        }
        defer { divi_free_string(ptr) }
        let json = String(cString: ptr)
        return try JSONDecoder().decode(Crest.self, from: Data(json.utf8))
    }

    /// Run the Arbiter layout solver.
    public static func resolveLayout(
        x: Double, y: Double, w: Double, h: Double,
        sanctumsJSON: String
    ) throws -> Domain {
        guard let ptr = divi_regalia_resolve_layout(x, y, w, h, sanctumsJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to resolve layout")
        }
        defer { divi_free_string(ptr) }
        let json = String(cString: ptr)
        return try JSONDecoder().decode(Domain.self, from: Data(json.utf8))
    }

    /// Get the Pulse dashboard sanctums as JSON (toolbar + sidebar + content).
    public static func pulseSanctums() throws -> String {
        guard let ptr = divi_regalia_pulse_sanctums() else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get pulse sanctums")
        }
        defer { divi_free_string(ptr) }
        return String(cString: ptr)
    }

    /// Convenience: resolve the Pulse dashboard layout for a given viewport.
    public static func resolvePulseLayout(width: Double, height: Double) throws -> Domain {
        let sanctumsJSON = try pulseSanctums()
        return try resolveLayout(x: 0, y: 0, w: width, h: height, sanctumsJSON: sanctumsJSON)
    }
}
