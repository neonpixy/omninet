import COmnideaFFI
import Foundation

// MARK: - Event Building (sync)

/// Globe event builder — creates and verifies signed OmniEvents.
public enum GlobeEvents {

    /// Build and sign a text note (kind 1).
    public static func textNote(_ content: String, keyring: Keyring) throws -> GlobeEvent {
        guard let json = divi_globe_event_text_note(content, keyring.ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create text note")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(GlobeEvent.self, from: data)
    }

    /// Sign an unsigned event with a keyring.
    public static func sign(kind: UInt32, content: String, tags: [[String]] = [], keyring: Keyring) throws -> GlobeEvent {
        let unsigned: [String: Any] = [
            "kind": kind,
            "content": content,
            "tags": tags,
        ]
        let jsonData = try JSONSerialization.data(withJSONObject: unsigned)
        let jsonString = String(data: jsonData, encoding: .utf8)!

        guard let json = divi_globe_event_sign(jsonString, keyring.ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to sign event")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(GlobeEvent.self, from: data)
    }

    /// Verify an event's ID and signature.
    public static func verify(_ event: GlobeEvent) -> Bool {
        guard let jsonData = try? JSONEncoder().encode(event),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            return false
        }
        return divi_globe_event_verify(jsonString)
    }
}

// MARK: - Filter Building (sync)

/// Globe filter builder — creates subscription filters.
public enum GlobeFilters {

    /// Filter for a user's profile (kind 0).
    public static func forProfile(pubkeyHex: String) -> String? {
        guard let json = divi_globe_filter_for_profile(pubkeyHex) else { return nil }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Filter for a user's contact list (kind 3).
    public static func forContactList(pubkeyHex: String) -> String? {
        guard let json = divi_globe_filter_for_contact_list(pubkeyHex) else { return nil }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Build a filter from JSON (validates and round-trips).
    public static func fromJSON(_ json: String) -> String? {
        guard let result = divi_globe_filter_from_json(json) else { return nil }
        defer { divi_free_string(result) }
        return String(cString: result)
    }
}

// MARK: - Relay Pool (async)

/// Multi-relay coordinator. Connects to relays, publishes events,
/// subscribes to streams, and deduplicates incoming events.
public final class GlobePool: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Create a new relay pool on the given runtime.
    public init(runtime: OmnideaRuntime, configJSON: String? = nil) {
        if let config = configJSON {
            ptr = divi_globe_pool_new(runtime.ptr, config)!
        } else {
            ptr = divi_globe_pool_new(runtime.ptr, nil)!
        }
    }

    deinit {
        divi_globe_pool_free(ptr)
    }

    /// Add a relay and connect. URL should be like "ws://localhost:8080".
    public func addRelay(_ url: String) throws {
        let result = divi_globe_pool_add_relay(ptr, url)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add relay '\(url)'")
        }
    }

    /// Publish an event to all connected relays.
    public func publish(_ event: GlobeEvent) throws {
        let jsonData = try JSONEncoder().encode(event)
        let jsonString = String(data: jsonData, encoding: .utf8)!
        let result = divi_globe_pool_publish(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to publish event")
        }
    }

    /// Subscribe to events matching the given filters (JSON array).
    /// Returns the subscription ID.
    public func subscribe(filtersJSON: String) throws -> String {
        guard let subId = divi_globe_pool_subscribe(ptr, filtersJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to subscribe")
        }
        defer { divi_free_string(subId) }
        return String(cString: subId)
    }

    /// Register a callback for incoming events.
    ///
    /// The callback fires on a background thread. Use `@MainActor` dispatch
    /// in your handler if you need to update UI.
    ///
    /// Only one callback can be active. Calling again replaces the previous one.
    public func onEvent(_ handler: @escaping (GlobeEvent, String) -> Void) {
        let boxed = Unmanaged.passRetained(GlobeEventBox(handler)).toOpaque()
        divi_globe_pool_on_event(ptr, globeEventTrampoline, boxed)
    }
}

// MARK: - Callback Internals

private final class GlobeEventBox: @unchecked Sendable {
    let handler: (GlobeEvent, String) -> Void
    init(_ handler: @escaping (GlobeEvent, String) -> Void) { self.handler = handler }
}

private let globeEventTrampoline: GlobeEventCallback = { eventJSON, sourceRelay, context in
    guard let context, let eventJSON, let sourceRelay else { return }
    let box_ = Unmanaged<GlobeEventBox>.fromOpaque(context).takeUnretainedValue()

    let jsonStr = String(cString: eventJSON)
    let relayStr = String(cString: sourceRelay)

    guard let data = jsonStr.data(using: .utf8),
          let event = try? JSONDecoder().decode(GlobeEvent.self, from: data) else {
        return
    }

    box_.handler(event, relayStr)
}

// MARK: - Relay Server

/// Globe relay server — every device can be a relay.
///
/// Starts a WebSocket server that accepts connections, stores events,
/// and broadcasts live events between connected clients.
public final class GlobeServer: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Start a relay server.
    ///
    /// - Parameters:
    ///   - runtime: The shared async runtime.
    ///   - port: TCP port to bind. Use 0 for OS-assigned.
    ///   - bindAll: If true, listens on all interfaces (reachable from LAN).
    ///              If false, localhost only.
    public init(runtime: OmnideaRuntime, port: UInt16 = 0, bindAll: Bool = true) throws {
        guard let p = divi_globe_server_start(runtime.ptr, port, bindAll, nil) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to start relay server")
        }
        ptr = p
    }

    deinit {
        divi_globe_server_free(ptr)
    }

    /// The port the server is listening on.
    public var port: UInt16 {
        divi_globe_server_port(ptr)
    }

    /// Number of active WebSocket connections.
    public var activeConnections: UInt32 {
        divi_globe_server_connections(ptr)
    }

    /// The server's WebSocket URL (e.g., "ws://0.0.0.0:52431").
    public var url: String {
        let cstr = divi_globe_server_url(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Inject an event directly into the store (bypasses WebSocket).
    /// Useful for seeding test data.
    public func seedEvent(_ event: GlobeEvent) throws {
        let jsonData = try JSONEncoder().encode(event)
        let jsonString = String(data: jsonData, encoding: .utf8)!
        let result = divi_globe_server_seed_event(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to seed event")
        }
    }
}

// MARK: - Codable Types

/// A signed, content-addressed ORP event.
public struct GlobeEvent: Codable, Sendable {
    public var id: String
    public var author: String
    public var createdAt: Int64
    public var kind: UInt32
    public var tags: [[String]]
    public var content: String
    public var sig: String

    enum CodingKeys: String, CodingKey {
        case id, author
        case createdAt = "created_at"
        case kind, tags, content, sig
    }
}
