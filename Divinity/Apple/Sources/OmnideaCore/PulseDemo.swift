import COmnideaFFI
import Foundation

// MARK: - Pulse demo report types

/// Result for a single crate demo step.
public struct CrateResult: Codable, Sendable {
    public let crateName: String
    public let letter: String
    public let success: Bool
    public let error: String?
    public let data: AnyCodable

    enum CodingKeys: String, CodingKey {
        case crateName = "crate_name"
        case letter, success, error, data
    }
}

/// The full Pulse demo report.
public struct PulseDemoReport: Codable, Sendable {
    public let version: String
    public let cratesTested: Int
    public let cratesPassed: Int
    public let results: [CrateResult]

    enum CodingKeys: String, CodingKey {
        case version
        case cratesTested = "crates_tested"
        case cratesPassed = "crates_passed"
        case results
    }
}

/// Type-erased Codable for arbitrary JSON data fields.
public struct AnyCodable: Codable, @unchecked Sendable {
    public let value: Any?

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = nil
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else if let arr = try? container.decode([AnyCodable].self) {
            value = arr.map { $0.value }
        } else if let str = try? container.decode(String.self) {
            value = str
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else {
            value = nil
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        if value == nil {
            try container.encodeNil()
        } else if let str = value as? String {
            try container.encode(str)
        } else if let bool = value as? Bool {
            try container.encode(bool)
        } else if let int = value as? Int {
            try container.encode(int)
        } else if let double = value as? Double {
            try container.encode(double)
        } else {
            try container.encodeNil()
        }
    }

    /// Get a string value from the data dictionary by key.
    public func string(forKey key: String) -> String? {
        (value as? [String: Any])?[key] as? String
    }

    /// Get a bool value from the data dictionary by key.
    public func bool(forKey key: String) -> Bool? {
        (value as? [String: Any])?[key] as? Bool
    }

    /// Get an int value from the data dictionary by key.
    public func int(forKey key: String) -> Int? {
        (value as? [String: Any])?[key] as? Int
    }
}

// MARK: - FFI bridge

/// Pulse FFI bridge — runs the full-stack demo.
public enum PulseFFI {

    /// Run the full Pulse demo across all 15 crates.
    public static func runDemo() throws -> PulseDemoReport {
        guard let ptr = divi_pulse_run_demo() else {
            try OmnideaError.check()
            throw OmnideaError(message: "Pulse demo returned null")
        }
        defer { divi_free_string(ptr) }
        let json = String(cString: ptr)
        return try JSONDecoder().decode(PulseDemoReport.self, from: Data(json.utf8))
    }
}
