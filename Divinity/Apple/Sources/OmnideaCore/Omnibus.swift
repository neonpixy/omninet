import COmnideaFFI
import Foundation

// MARK: - Omnibus Status

/// Current status of an Omnibus instance.
public struct OmnibusStatus: Codable, Sendable {
    public let hasIdentity: Bool
    public let pubkey: String?
    public let displayName: String?
    public let relayPort: UInt16
    public let relayConnections: UInt32
    public let relayUrl: String
    public let discoveredPeers: UInt32
    public let poolRelays: UInt32
    public let hasHomeNode: Bool
    public let publicUrl: String?

    enum CodingKeys: String, CodingKey {
        case hasIdentity = "has_identity"
        case pubkey
        case displayName = "display_name"
        case relayPort = "relay_port"
        case relayConnections = "relay_connections"
        case relayUrl = "relay_url"
        case discoveredPeers = "discovered_peers"
        case poolRelays = "pool_relays"
        case hasHomeNode = "has_home_node"
        case publicUrl = "public_url"
    }
}

// MARK: - Store Stats

/// Event store statistics from the local relay.
public struct StoreStats: Codable, Sendable {
    public let eventCount: Int
    public let oldestEvent: Int64?
    public let newestEvent: Int64?
    public let eventsByKind: [String: Int]

    enum CodingKeys: String, CodingKey {
        case eventCount = "event_count"
        case oldestEvent = "oldest_event"
        case newestEvent = "newest_event"
        case eventsByKind = "events_by_kind"
    }
}

// MARK: - Relay Health Snapshot

/// Health summary for a connected relay.
public struct RelayHealthSnapshot: Codable, Sendable, Identifiable {
    public var id: String { url }
    public let url: String
    public let state: String
    public let connectedSince: String?
    public let lastActivity: String?
    public let sendCount: UInt64
    public let receiveCount: UInt64
    public let errorCount: UInt64
    public let averageLatencyMs: Double?
    public let score: Double

    enum CodingKeys: String, CodingKey {
        case url, state, score
        case connectedSince = "connected_since"
        case lastActivity = "last_activity"
        case sendCount = "send_count"
        case receiveCount = "receive_count"
        case errorCount = "error_count"
        case averageLatencyMs = "average_latency_ms"
    }
}

// MARK: - Log Entry

/// A captured log entry from the node runtime.
public struct LogEntry: Codable, Sendable, Identifiable {
    public var id: String { "\(timestamp)-\(message.hashValue)" }
    public let timestamp: String
    public let level: String
    public let module: String?
    public let message: String
}

// MARK: - Omnibus

/// The shared node runtime. Every Throne app creates one.
///
/// Omnibus boots a local relay server, mDNS discovery, identity management,
/// and a relay pool — all in one call. It's the engine that makes your device
/// a node in the Omnidea web.
///
/// ```swift
/// let omnibus = try Omnibus(deviceName: "Sam's Mac")
/// let crownId = try omnibus.createIdentity(displayName: "Sam")
/// try omnibus.post("Hello from Omnidea!")
/// ```
public final class Omnibus: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Start an Omnibus instance.
    ///
    /// - Parameters:
    ///   - deviceName: Human-readable name for mDNS discovery.
    ///   - port: Relay server port. 0 = OS-assigned (default).
    ///   - bindAll: Listen on all interfaces for LAN reachability (default true).
    public init(deviceName: String, port: UInt16 = 0, bindAll: Bool = true) throws {
        guard let p = divi_omnibus_start(deviceName, port, bindAll) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to start Omnibus")
        }
        ptr = p
    }

    deinit {
        divi_omnibus_free(ptr)
    }

    // MARK: - Identity

    /// Create a new identity with a display name.
    /// Returns the crownId (bech32 public key).
    @discardableResult
    public func createIdentity(displayName: String) throws -> String {
        guard let crownId = divi_omnibus_create_identity(ptr, displayName) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create identity")
        }
        defer { divi_free_string(crownId) }
        return String(cString: crownId)
    }

    /// The public key (crownId bech32), or nil if no identity loaded.
    public var pubkey: String? {
        guard let pk = divi_omnibus_pubkey(ptr) else { return nil }
        defer { divi_free_string(pk) }
        return String(cString: pk)
    }

    /// The public key as hex, or nil if no identity loaded.
    public var pubkeyHex: String? {
        guard let pk = divi_omnibus_pubkey_hex(ptr) else { return nil }
        defer { divi_free_string(pk) }
        return String(cString: pk)
    }

    /// The current profile as JSON, or nil if no identity loaded.
    public var profileJSON: String? {
        guard let json = divi_omnibus_profile_json(ptr) else { return nil }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Update the display name and re-publish the profile.
    public func updateDisplayName(_ name: String) throws {
        let result = divi_omnibus_update_display_name(ptr, name)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update display name")
        }
    }

    /// Export the internal keyring as bytes (for syncing to a standalone Keyring).
    public func exportKeyring() throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = divi_omnibus_export_keyring(ptr, &outData, &outLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to export keyring")
        }

        guard let outData else { return Data() }
        let data = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return data
    }

    /// Import a keyring from exported bytes (for syncing from another device).
    public func importKeyring(_ data: Data) throws {
        let result = data.withUnsafeBytes { buffer in
            divi_omnibus_import_keyring(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to import keyring")
        }
    }

    /// Load an existing identity from a directory path.
    /// The path should contain a `soul/` subdirectory and optionally `keyring.dat`.
    /// Returns the crownId (bech32 public key).
    @discardableResult
    public func loadIdentity(from path: String) throws -> String {
        guard let crownId = divi_omnibus_load_identity(ptr, path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to load identity from '\(path)'")
        }
        defer { divi_free_string(crownId) }
        return String(cString: crownId)
    }

    // MARK: - Network

    /// Post a text note. Signs it with the loaded identity and publishes.
    /// Returns the signed event.
    @discardableResult
    public func post(_ content: String) throws -> GlobeEvent {
        guard let json = divi_omnibus_post(ptr, content) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to post")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(GlobeEvent.self, from: data)
    }

    /// Publish a pre-signed event to all connected relays.
    public func publish(_ event: GlobeEvent) throws {
        let jsonData = try JSONEncoder().encode(event)
        let jsonString = String(data: jsonData, encoding: .utf8)!
        let result = divi_omnibus_publish(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to publish event")
        }
    }

    /// Inject an event directly into the local relay store (bypasses network).
    public func seedEvent(_ event: GlobeEvent) throws {
        let jsonData = try JSONEncoder().encode(event)
        let jsonString = String(data: jsonData, encoding: .utf8)!
        let result = divi_omnibus_seed_event(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to seed event")
        }
    }

    /// Connect to a specific relay.
    public func connectRelay(_ url: String) throws {
        let result = divi_omnibus_connect_relay(ptr, url)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to connect to relay '\(url)'")
        }
    }

    /// Set a home node for persistent sync.
    public func setHomeNode(_ url: String) throws {
        let result = divi_omnibus_set_home_node(ptr, url)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set home node")
        }
    }

    /// Query events from the local relay store.
    public func query(filterJSON: String) throws -> [GlobeEvent] {
        guard let json = divi_omnibus_query(ptr, filterJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query events")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([GlobeEvent].self, from: data)
    }

    // MARK: - Events

    /// Subscribe to events matching filters.
    /// Returns the subscription ID.
    public func subscribe(filtersJSON: String) throws -> String {
        guard let subId = divi_omnibus_subscribe(ptr, filtersJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to subscribe")
        }
        defer { divi_free_string(subId) }
        return String(cString: subId)
    }

    /// Register a callback for ALL incoming events.
    ///
    /// The callback fires on a background thread. Use `@MainActor` dispatch
    /// in your handler if you need to update UI.
    ///
    /// Only one callback can be active — calling again replaces the previous one.
    public func onEvent(_ handler: @escaping (GlobeEvent, String) -> Void) {
        let boxed = Unmanaged.passRetained(OmnibusEventBox(handler)).toOpaque()
        divi_omnibus_on_event(ptr, omnibusEventTrampoline, boxed)
    }

    // MARK: - Discovery

    /// All currently discovered peers on the local network.
    public var peers: [LocalPeer] {
        guard let json = divi_omnibus_peers(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([LocalPeer].self, from: data)) ?? []
    }

    /// Number of currently discovered peers.
    public var peerCount: UInt32 {
        divi_omnibus_peer_count(ptr)
    }

    /// Connect to all currently discovered peers.
    /// Returns the number of peers connected.
    @discardableResult
    public func connectDiscoveredPeers() -> UInt32 {
        divi_omnibus_connect_discovered_peers(ptr)
    }

    // MARK: - Status

    /// Full status of this Omnibus instance.
    public var status: OmnibusStatus? {
        guard let json = divi_omnibus_status(ptr) else { return nil }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try? JSONDecoder().decode(OmnibusStatus.self, from: data)
    }

    /// The local relay server port.
    public var port: UInt16 {
        divi_omnibus_port(ptr)
    }

    /// The local relay server WebSocket URL.
    public var relayURL: String {
        let cstr = divi_omnibus_relay_url(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The public URL of this node (if UPnP mapping succeeded), or nil if local only.
    public var publicURL: String? {
        guard let cstr = divi_omnibus_public_url(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    // MARK: - Health & Diagnostics

    /// Event store statistics.
    public var storeStats: StoreStats? {
        guard let json = divi_omnibus_store_stats(ptr) else { return nil }
        defer { divi_free_string(json) }
        return try? JSONDecoder().decode(StoreStats.self, from: Data(String(cString: json).utf8))
    }

    /// Health snapshots for all connected relays.
    public var relayHealth: [RelayHealthSnapshot] {
        guard let json = divi_omnibus_relay_health(ptr) else { return [] }
        defer { divi_free_string(json) }
        return (try? JSONDecoder().decode([RelayHealthSnapshot].self, from: Data(String(cString: json).utf8))) ?? []
    }

    /// Health snapshot for a specific relay URL.
    public func relayHealth(for url: String) -> RelayHealthSnapshot? {
        guard let json = divi_omnibus_relay_health_for(ptr, url) else { return nil }
        defer { divi_free_string(json) }
        return try? JSONDecoder().decode(RelayHealthSnapshot.self, from: Data(String(cString: json).utf8))
    }

    // MARK: - Log Capture

    /// Get the most recent log entries.
    public func recentLogs(count: UInt32 = 50) -> [LogEntry] {
        guard let json = divi_omnibus_recent_logs(ptr, count) else { return [] }
        defer { divi_free_string(json) }
        return (try? JSONDecoder().decode([LogEntry].self, from: Data(String(cString: json).utf8))) ?? []
    }

    /// Push a log entry into the capture buffer.
    @discardableResult
    public func pushLog(level: String, module: String? = nil, message: String) -> Bool {
        divi_omnibus_push_log(ptr, level, module, message) == 0
    }
}

// MARK: - Callback Internals

private final class OmnibusEventBox: @unchecked Sendable {
    let handler: (GlobeEvent, String) -> Void
    init(_ handler: @escaping (GlobeEvent, String) -> Void) { self.handler = handler }
}

private let omnibusEventTrampoline: OmnibusEventCallback = { eventJSON, sourceRelay, context in
    guard let context, let eventJSON, let sourceRelay else { return }
    let box_ = Unmanaged<OmnibusEventBox>.fromOpaque(context).takeUnretainedValue()

    let jsonStr = String(cString: eventJSON)
    let relayStr = String(cString: sourceRelay)

    guard let data = jsonStr.data(using: .utf8),
          let event = try? JSONDecoder().decode(GlobeEvent.self, from: data) else {
        return
    }

    box_.handler(event, relayStr)
}
