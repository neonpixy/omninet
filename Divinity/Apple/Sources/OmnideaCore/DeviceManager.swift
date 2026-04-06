import COmnideaFFI
import Foundation

// MARK: - DevicePairing

/// Stateless device pairing protocol.
///
/// Wraps the three-step pairing handshake: initiate, respond, verify.
/// All methods are static -- no state to manage. Each returns raw JSON
/// that the caller can decode as needed.
///
/// ```swift
/// let challenge = try DevicePairing.initiate(
///     secretKeyHex: myHex, deviceName: "MacBook", relayURL: "wss://relay.example.com"
/// )
/// let response = try DevicePairing.respond(
///     challengeJSON: challenge, secretKeyHex: otherHex, deviceName: "iPhone"
/// )
/// let pair = try DevicePairing.verify(challengeJSON: challenge, responseJSON: response)
/// ```
public enum DevicePairing {

    /// Initiate a pairing challenge.
    ///
    /// - Parameters:
    ///   - secretKeyHex: The initiator's 32-byte private key as a 64-char hex string.
    ///   - deviceName: Human-readable name for this device.
    ///   - relayURL: Relay URL for the pairing response.
    /// - Returns: JSON string (PairingChallenge).
    public static func initiate(
        secretKeyHex: String,
        deviceName: String,
        relayURL: String
    ) throws -> String {
        guard let cstr = divi_device_pairing_initiate(secretKeyHex, deviceName, relayURL) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to initiate device pairing")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Respond to a pairing challenge by signing the nonce.
    ///
    /// - Parameters:
    ///   - challengeJSON: JSON string (PairingChallenge) from the initiator.
    ///   - secretKeyHex: The responder's 32-byte private key as a 64-char hex string.
    ///   - deviceName: Human-readable name for this device.
    /// - Returns: JSON string (PairingResponse).
    public static func respond(
        challengeJSON: String,
        secretKeyHex: String,
        deviceName: String
    ) throws -> String {
        guard let cstr = divi_device_pairing_respond(challengeJSON, secretKeyHex, deviceName) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to respond to device pairing")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Verify a pairing response and produce a DevicePair.
    ///
    /// - Parameters:
    ///   - challengeJSON: JSON string (PairingChallenge).
    ///   - responseJSON: JSON string (PairingResponse).
    /// - Returns: JSON string (DevicePair).
    public static func verify(
        challengeJSON: String,
        responseJSON: String
    ) throws -> String {
        guard let cstr = divi_device_pairing_verify(challengeJSON, responseJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to verify device pairing")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - DeviceFleet

/// A registry of paired devices.
///
/// Wraps the Rust `DeviceFleet` behind an opaque pointer. Thread-safe
/// (Mutex on the Rust side). Manages device entries, status updates,
/// and fleet health checks.
///
/// ```swift
/// let fleet = DeviceFleet()
/// try fleet.add(entryJSON: deviceJSON)
/// let all = fleet.list()
/// let health = fleet.health()
/// ```
public final class DeviceFleet: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_device_fleet_new()!
    }

    private init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_device_fleet_free(ptr)
    }

    /// Add a device to the fleet.
    ///
    /// - Parameter entryJSON: JSON string (FleetEntry).
    public func add(entryJSON: String) throws {
        let result = divi_device_fleet_add(ptr, entryJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add device to fleet")
        }
    }

    /// Remove a device from the fleet by crownId.
    ///
    /// - Parameter crownId: The device's public key.
    /// - Returns: JSON string (FleetEntry) of the removed device, or nil if not found.
    public func remove(crownId: String) throws -> String? {
        guard let cstr = divi_device_fleet_remove(ptr, crownId) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a device by crownId.
    ///
    /// - Parameter crownId: The device's public key.
    /// - Returns: JSON string (FleetEntry), or nil if not found.
    public func get(crownId: String) -> String? {
        guard let cstr = divi_device_fleet_get(ptr, crownId) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// List all devices in the fleet.
    ///
    /// - Returns: JSON string (array of FleetEntry).
    public func list() -> String {
        let cstr = divi_device_fleet_list(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Update the status of a device.
    ///
    /// - Parameters:
    ///   - crownId: The device's public key.
    ///   - statusJSON: JSON string (DeviceStatus).
    public func updateStatus(crownId: String, statusJSON: String) throws {
        let result = divi_device_fleet_update_status(ptr, crownId, statusJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update device status for '\(crownId)'")
        }
    }

    /// Get aggregate fleet health.
    ///
    /// - Returns: JSON string (FleetHealth).
    public func health() -> String {
        let cstr = divi_device_fleet_health(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of devices in the fleet.
    public var count: Int {
        Int(divi_device_fleet_count(ptr))
    }

    /// Whether the fleet has no devices.
    public var isEmpty: Bool {
        divi_device_fleet_is_empty(ptr)
    }

    /// Serialize the fleet to JSON.
    ///
    /// - Returns: JSON string (DeviceFleet).
    public func toJSON() -> String {
        let cstr = divi_device_fleet_to_json(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Deserialize a fleet from JSON.
    ///
    /// - Parameter json: JSON string (DeviceFleet).
    /// - Returns: A new `DeviceFleet`, or nil on error.
    public static func fromJSON(_ json: String) throws -> DeviceFleet? {
        guard let raw = divi_device_fleet_from_json(json) else {
            try OmnideaError.check()
            return nil
        }
        return DeviceFleet(ptr: raw)
    }
}

// MARK: - SyncPriority

/// Maps data types to their "home" device for sync priority.
///
/// Wraps the Rust `SyncPriority` behind an opaque pointer. Thread-safe
/// (Mutex on the Rust side). Determines which device is authoritative
/// for each data type.
///
/// ```swift
/// let priority = SyncPriority()
/// priority.setHome(dataType: "notes", deviceCrownId: "cpub1...")
/// let home = priority.homeFor(dataType: "notes")
/// ```
public final class SyncPriority: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_device_sync_priority_new()!
    }

    private init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_device_sync_priority_free(ptr)
    }

    /// Set which device is home for a data type.
    ///
    /// - Parameters:
    ///   - dataType: The data type identifier.
    ///   - deviceCrownId: The device's public key.
    public func setHome(dataType: String, deviceCrownId: String) {
        divi_device_sync_priority_set_home(ptr, dataType, deviceCrownId)
    }

    /// Get the home device for a data type.
    ///
    /// - Parameter dataType: The data type identifier.
    /// - Returns: The device crownId, or nil if no home is set.
    public func homeFor(dataType: String) -> String? {
        guard let cstr = divi_device_sync_priority_home_for(ptr, dataType) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Remove the home assignment for a data type.
    ///
    /// - Parameter dataType: The data type identifier.
    public func remove(dataType: String) {
        divi_device_sync_priority_remove(ptr, dataType)
    }

    /// Get all home assignments.
    ///
    /// - Returns: JSON string (object mapping data_type -> device_crown_id).
    public func all() -> String {
        let cstr = divi_device_sync_priority_all(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Serialize the sync priority map to JSON.
    ///
    /// - Returns: JSON string (SyncPriority).
    public func toJSON() -> String {
        let cstr = divi_device_sync_priority_to_json(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Deserialize a sync priority map from JSON.
    ///
    /// - Parameter json: JSON string (SyncPriority).
    /// - Returns: A new `SyncPriority`, or nil on error.
    public static func fromJSON(_ json: String) throws -> SyncPriority? {
        guard let raw = divi_device_sync_priority_from_json(json) else {
            try OmnideaError.check()
            return nil
        }
        return SyncPriority(ptr: raw)
    }
}

// MARK: - SyncTracker

/// Tracks sync state per device per data type.
///
/// Wraps the Rust `SyncTracker` behind an opaque pointer. Thread-safe
/// (Mutex on the Rust side). Monitors whether each device is synced,
/// pending, or in conflict for each data type.
///
/// ```swift
/// let tracker = SyncTracker()
/// try tracker.setState(crownId: "cpub1...", dataType: "notes", stateJSON: stateJSON)
/// let synced = tracker.allSynced
/// let conflicts = tracker.conflicts()
/// ```
public final class SyncTracker: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_device_sync_tracker_new()!
    }

    private init(ptr: OpaquePointer) {
        self.ptr = ptr
    }

    deinit {
        divi_device_sync_tracker_free(ptr)
    }

    /// Set the sync state for a device + data type pair.
    ///
    /// - Parameters:
    ///   - crownId: The device's public key.
    ///   - dataType: The data type identifier.
    ///   - stateJSON: JSON string (SyncState).
    public func setState(crownId: String, dataType: String, stateJSON: String) throws {
        let result = divi_device_sync_tracker_set_state(ptr, crownId, dataType, stateJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set sync state for '\(crownId)' / '\(dataType)'")
        }
    }

    /// Get the sync state for a device + data type pair.
    ///
    /// Returns "Unknown" (as JSON) if not tracked.
    ///
    /// - Parameters:
    ///   - crownId: The device's public key.
    ///   - dataType: The data type identifier.
    /// - Returns: JSON string (SyncState).
    public func getState(crownId: String, dataType: String) -> String? {
        guard let cstr = divi_device_sync_tracker_get_state(ptr, crownId, dataType) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get all sync states for a specific device.
    ///
    /// - Parameter crownId: The device's public key.
    /// - Returns: JSON string (object mapping data_type -> SyncState), or nil if not tracked.
    public func statesForDevice(crownId: String) -> String? {
        guard let cstr = divi_device_sync_tracker_states_for_device(ptr, crownId) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Whether all tracked states are synced (or the tracker is empty).
    public var allSynced: Bool {
        divi_device_sync_tracker_all_synced(ptr)
    }

    /// Get all conflict states.
    ///
    /// - Returns: JSON string (array of objects with `device_crown_id`, `data_type`, `state`).
    public func conflicts() -> String {
        let cstr = divi_device_sync_tracker_conflicts(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Serialize the sync tracker to JSON.
    ///
    /// - Returns: JSON string (SyncTracker).
    public func toJSON() -> String {
        let cstr = divi_device_sync_tracker_to_json(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Deserialize a sync tracker from JSON.
    ///
    /// - Parameter json: JSON string (SyncTracker).
    /// - Returns: A new `SyncTracker`, or nil on error.
    public static func fromJSON(_ json: String) throws -> SyncTracker? {
        guard let raw = divi_device_sync_tracker_from_json(json) else {
            try OmnideaError.check()
            return nil
        }
        return SyncTracker(ptr: raw)
    }
}
