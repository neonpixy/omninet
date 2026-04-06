import COmnideaFFI
import Foundation

// MARK: - Trust Graph (stateful)

/// Jail trust graph — tracks verification edges between people.
public final class TrustGraph: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_jail_trust_graph_new()!
    }

    deinit {
        divi_jail_trust_graph_free(ptr)
    }

    /// Add a verification edge (JSON VerificationEdge).
    public func addEdge(_ edgeJSON: String) throws {
        let result = divi_jail_trust_graph_add_edge(ptr, edgeJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add edge")
        }
    }

    /// Remove a verification edge by UUID string.
    public func removeEdge(id: String) throws {
        let result = divi_jail_trust_graph_remove_edge(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove edge")
        }
    }

    /// Query network intelligence about a target person.
    /// `flagsJSON` is a JSON array of AccountabilityFlags.
    /// Returns JSON NetworkIntelligence.
    public func queryIntelligence(
        querier: String, target: String, flagsJSON: String, configJSON: String? = nil
    ) throws -> String {
        guard let json = divi_jail_trust_graph_query_intelligence(
            ptr, querier, target, flagsJSON, configJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to query intelligence")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Number of people (nodes) in the graph.
    public var nodeCount: UInt32 {
        divi_jail_trust_graph_node_count(ptr)
    }

    /// Number of verification edges in the graph.
    public var edgeCount: UInt32 {
        divi_jail_trust_graph_edge_count(ptr)
    }

    /// Check admission for a prospect to a community.
    /// Returns JSON AdmissionRecommendation.
    public func checkAdmission(
        prospect: String, communityId: String,
        membersJSON: String, flagsJSON: String, configJSON: String? = nil
    ) throws -> String {
        guard let json = divi_jail_check_admission(
            ptr, prospect, communityId, membersJSON, flagsJSON, configJSON
        ) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check admission")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Accountability Flags

public enum JailFlags {

    /// Raise an accountability flag.
    /// `category` is JSON like `"Harassment"`. `severity` is JSON like `"High"`.
    /// Returns JSON AccountabilityFlag.
    public static func raise(
        flagger: String, flagged: String,
        category: String, severity: String, description: String
    ) throws -> String {
        guard let json = divi_jail_flag_raise(flagger, flagged, category, severity, description) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to raise flag")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Graduated Response

public enum JailResponses {

    /// Begin a graduated response against a target.
    /// Returns JSON GraduatedResponse.
    public static func begin(target: String, reason: String, initiatedBy: String) throws -> String {
        guard let json = divi_jail_response_begin(target, reason, initiatedBy) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to begin response")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Escalate a graduated response. Returns updated response JSON.
    public static func escalate(responseJSON: String, reason: String, initiatedBy: String) throws -> String {
        guard let json = divi_jail_response_escalate(responseJSON, reason, initiatedBy) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to escalate response")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// De-escalate a graduated response. Returns updated response JSON.
    public static func deEscalate(responseJSON: String, reason: String, initiatedBy: String) throws -> String {
        guard let json = divi_jail_response_de_escalate(responseJSON, reason, initiatedBy) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to de-escalate response")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Config

public enum JailConfiguration {

    /// Get the default JailConfig as JSON.
    public static func defaultConfig() -> String {
        let json = divi_jail_config_default()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
