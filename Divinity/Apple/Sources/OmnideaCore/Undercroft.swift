import COmnideaFFI
import Foundation

// MARK: - Undercroft

/// Stateless health aggregation functions from the Undercroft observatory.
///
/// Undercroft is the vaulted chamber beneath the castle -- it observes system
/// health, network topology, and economic vitals. It never controls, only reports.
///
/// All functions accept and return raw JSON strings. The caller is responsible
/// for constructing valid JSON inputs (matching the Rust serde types) and
/// interpreting the JSON outputs.
public enum Undercroft {

    // MARK: - Network Health

    /// Validate and round-trip a NetworkHealth JSON.
    ///
    /// Use this to deserialize a pre-computed `NetworkHealth` on the Rust side
    /// and re-serialize it back. The aggregation from relay data to NetworkHealth
    /// must happen in Rust before crossing the FFI boundary.
    ///
    /// - Parameter networkHealthJSON: JSON `NetworkHealth`.
    /// - Returns: JSON `NetworkHealth`.
    public static func networkHealth(_ networkHealthJSON: String) throws -> String {
        guard let json = divi_undercroft_network_health(networkHealthJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute network health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create an empty NetworkHealth snapshot.
    ///
    /// - Returns: JSON `NetworkHealth` with zeroed values.
    public static func networkHealthEmpty() -> String {
        let json = divi_undercroft_network_health_empty()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    // MARK: - Community Health

    /// Aggregate community health from Kingdom and Bulwark data.
    ///
    /// - Parameters:
    ///   - communityJSON: JSON `kingdom::Community`.
    ///   - proposalsJSON: JSON array of `kingdom::Proposal`.
    ///   - pulseJSON: Optional JSON `bulwark::CollectiveHealthPulse`. Pass `nil` if unavailable.
    /// - Returns: JSON `CommunityHealth`.
    public static func communityHealth(
        communityJSON: String,
        proposalsJSON: String,
        pulseJSON: String? = nil
    ) throws -> String {
        guard let json = divi_undercroft_community_health(
            communityJSON,
            proposalsJSON,
            pulseJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute community health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    // MARK: - Economic Health

    /// Aggregate economic health from a Fortune TreasuryStatus.
    ///
    /// - Parameter treasuryStatusJSON: JSON `fortune::TreasuryStatus`.
    /// - Returns: JSON `EconomicHealth`.
    public static func economicHealth(_ treasuryStatusJSON: String) throws -> String {
        guard let json = divi_undercroft_economic_health(treasuryStatusJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute economic health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create an empty EconomicHealth snapshot.
    ///
    /// - Returns: JSON `EconomicHealth` with zeroed values.
    public static func economicHealthEmpty() -> String {
        let json = divi_undercroft_economic_health_empty()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    // MARK: - Health Metrics

    /// Compute top-level health metrics from a snapshot.
    ///
    /// - Parameters:
    ///   - snapshotJSON: JSON `HealthSnapshot`.
    ///   - nodeCount: Estimated number of network nodes.
    ///   - storeStatsJSON: Optional JSON `globe::StoreStats`. Pass `nil` if unavailable.
    /// - Returns: JSON `HealthMetrics`.
    public static func healthMetrics(
        snapshotJSON: String,
        nodeCount: UInt64,
        storeStatsJSON: String? = nil
    ) throws -> String {
        guard let json = divi_undercroft_health_metrics(
            snapshotJSON,
            nodeCount,
            storeStatsJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute health metrics")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    // MARK: - Privacy Health

    /// Validate and round-trip a RelayPrivacyHealth JSON.
    ///
    /// - Parameter healthJSON: JSON `RelayPrivacyHealth`.
    /// - Returns: JSON `RelayPrivacyHealth`.
    public static func privacyHealth(_ healthJSON: String) throws -> String {
        guard let json = divi_undercroft_privacy_health(healthJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute privacy health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Compute the privacy health score from a RelayPrivacyHealth JSON.
    ///
    /// - Parameter healthJSON: JSON `RelayPrivacyHealth`.
    /// - Returns: Health score between 0.0 and 1.0.
    public static func privacyHealthScore(_ healthJSON: String) throws -> Double {
        let score = divi_undercroft_privacy_health_score(healthJSON)
        if score < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute privacy health score")
        }
        return score
    }

    // MARK: - Quest Health

    /// Build quest health from an ObservatoryReport JSON.
    ///
    /// - Parameter reportJSON: JSON `quest::ObservatoryReport`.
    /// - Returns: JSON `QuestHealth`.
    public static func questHealth(_ reportJSON: String) throws -> String {
        guard let json = divi_undercroft_quest_health(reportJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute quest health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create an empty QuestHealth snapshot.
    ///
    /// - Returns: JSON `QuestHealth` with zeroed values.
    public static func questHealthEmpty() -> String {
        let json = divi_undercroft_quest_health_empty()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Compute the quest health score from a QuestHealth JSON.
    ///
    /// - Parameter questHealthJSON: JSON `QuestHealth`.
    /// - Returns: Health score between 0.0 and 1.0.
    public static func questHealthScore(_ questHealthJSON: String) throws -> Double {
        let score = divi_undercroft_quest_health_score(questHealthJSON)
        if score < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute quest health score")
        }
        return score
    }
}

// MARK: - HealthHistory

/// A ring buffer of `HealthSnapshot` entries, backed by an opaque Rust pointer.
///
/// HealthHistory stores up to `capacity` snapshots, evicting the oldest when full.
/// Default capacity is 168 (one week of hourly snapshots).
///
/// ```swift
/// let history = HealthHistory()
/// try history.push(snapshotJSON: someSnapshot)
/// let latest = history.latest()     // most recent snapshot or nil
/// let all = history.all()           // oldest to newest
/// ```
public final class HealthHistory: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Create a health history with a specific capacity.
    ///
    /// - Parameter capacity: Maximum number of snapshots to retain.
    public init(capacity: Int) {
        ptr = divi_undercroft_history_new(UInt(capacity))!
    }

    /// Create a health history with default capacity (168 = 1 week of hourly snapshots).
    public init() {
        ptr = divi_undercroft_history_default()!
    }

    deinit {
        divi_undercroft_history_free(ptr)
    }

    /// Push a snapshot into the history. Evicts the oldest if at capacity.
    ///
    /// - Parameter snapshotJSON: JSON `HealthSnapshot`.
    public func push(snapshotJSON: String) throws {
        let result = divi_undercroft_history_push(ptr, snapshotJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to push snapshot to history")
        }
    }

    /// Get the most recent snapshot, if any.
    ///
    /// - Returns: JSON `HealthSnapshot`, or `nil` if empty.
    public func latest() -> String? {
        guard let json = divi_undercroft_history_latest(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// The number of snapshots currently in the history.
    public var count: Int {
        Int(divi_undercroft_history_len(ptr))
    }

    /// Whether the history contains no snapshots.
    public var isEmpty: Bool {
        divi_undercroft_history_is_empty(ptr)
    }

    /// Get all snapshots as a JSON array, ordered oldest to newest.
    ///
    /// - Returns: JSON array of `HealthSnapshot`.
    public func all() -> String {
        let json = divi_undercroft_history_all(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
