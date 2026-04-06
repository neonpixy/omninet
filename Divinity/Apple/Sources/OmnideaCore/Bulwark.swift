import COmnideaFFI
import Foundation

// MARK: - Trust

/// Bulwark trust and safety operations.
public enum BulwarkTrust {

    /// Get capabilities for a trust layer. `layer` is JSON like `"Verified"`.
    /// Returns JSON LayerCapabilities.
    public static func layerCapabilities(_ layer: String) throws -> String {
        guard let json = divi_bulwark_trust_layer_capabilities(layer) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get layer capabilities")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create a new visible bond. `depth` is JSON like `"Friend"`.
    /// Returns JSON VisibleBond.
    public static func createBond(partyA: String, partyB: String, depth: String) throws -> String {
        guard let json = divi_bulwark_bond_new(partyA, partyB, depth) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create bond")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Update a bond's depth from one party. Returns updated bond JSON.
    public static func updateBondDepth(bondJSON: String, pubkey: String, newDepth: String) throws -> String {
        guard let json = divi_bulwark_bond_update_depth(bondJSON, pubkey, newDepth) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update bond depth")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Health

public enum BulwarkHealth {

    /// Compute a user health pulse. `factors` is JSON UserHealthFactors.
    /// Returns JSON UserHealthPulse.
    public static func computeUserHealth(pubkey: String, factors: String) throws -> String {
        guard let json = divi_bulwark_user_health_compute(pubkey, factors) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute user health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Compute a collective health pulse. `factors` is JSON CollectiveHealthFactors.
    /// Returns JSON CollectiveHealthPulse.
    public static func computeCollectiveHealth(
        collectiveId: String, factors: String, contributingMembers: UInt32
    ) throws -> String {
        guard let json = divi_bulwark_collective_health_compute(collectiveId, factors, contributingMembers) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to compute collective health")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Reputation

public enum BulwarkReputation {

    /// Create a new reputation for a pubkey (default scores).
    /// Returns JSON Reputation.
    public static func create(pubkey: String) throws -> String {
        guard let json = divi_bulwark_reputation_new(pubkey) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create reputation")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Apply a reputation event. Returns updated reputation JSON.
    public static func applyEvent(reputationJSON: String, eventJSON: String) throws -> String {
        guard let json = divi_bulwark_reputation_apply_event(reputationJSON, eventJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to apply reputation event")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Child Safety (Covenant-mandated)

public enum BulwarkChildSafety {

    /// File a child safety flag. `concern` is JSON like `"Grooming"`.
    /// Returns JSON ChildSafetyFlag.
    public static func fileFlag(reporter: String, concern: String, description: String) throws -> String {
        guard let json = divi_bulwark_child_safety_flag_file(reporter, concern, description) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to file child safety flag")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the default child safety protocol (all protections enabled).
    public static func defaultProtocol() -> String {
        let json = divi_bulwark_child_safety_protocol_default()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get US default real-world resources (911, 988, Childhelp).
    public static func realWorldResources() -> String {
        let json = divi_bulwark_real_world_resources()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
