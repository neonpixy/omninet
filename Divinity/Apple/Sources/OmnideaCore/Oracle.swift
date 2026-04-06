import COmnideaFFI
import Foundation

// MARK: - Disclosure Tracker (JSON round-trip)

/// Tracks a user's sovereignty tier progression based on recorded signals.
///
/// All state is carried as JSON -- create a tracker, record signals, and
/// persist the returned JSON string. No opaque pointers needed.
public enum OracleDisclosureTracker {

    /// Create a new tracker at the default Citizen tier.
    public static func new() -> String {
        let json = divi_oracle_disclosure_tracker_new()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create a tracker with a custom disclosure config (JSON DisclosureConfig).
    public static func withConfig(configJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_with_config(configJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create disclosure tracker with config")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Record a disclosure signal on a tracker.
    ///
    /// Returns the modified tracker JSON (carry it forward).
    public static func record(trackerJSON: String, signalJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_record(trackerJSON, signalJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record disclosure signal")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the current sovereignty tier from a tracker.
    ///
    /// Returns JSON SovereigntyTier (e.g. `"Citizen"`).
    public static func level(trackerJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_level(trackerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get disclosure level")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Manually set the sovereignty tier on a tracker.
    ///
    /// `levelJSON` is a JSON SovereigntyTier (e.g. `"Architect"`).
    /// Returns the modified tracker JSON.
    public static func setLevel(trackerJSON: String, levelJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_set_level(trackerJSON, levelJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set disclosure level")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Clear the manual override, returning to behavior-driven tier.
    ///
    /// Returns the modified tracker JSON.
    public static func clearOverride(trackerJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_clear_override(trackerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to clear disclosure override")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get signal counts from a tracker.
    ///
    /// Returns JSON `{"steward":<n>,"architect":<n>}`.
    public static func signalCounts(trackerJSON: String) throws -> String {
        guard let json = divi_oracle_disclosure_tracker_signal_counts(trackerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get signal counts")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Tier Defaults

/// Default settings and feature visibility per sovereignty tier.
public enum OracleTierDefaults {

    /// Get default settings for a sovereignty tier.
    ///
    /// `tierJSON` is a JSON SovereigntyTier (e.g. `"Citizen"`).
    /// Returns JSON TierDefaults.
    public static func defaults(tierJSON: String) throws -> String {
        guard let json = divi_oracle_tier_defaults(tierJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get tier defaults")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all tier defaults (one per sovereignty tier).
    ///
    /// Returns JSON array of TierDefaults.
    public static func all() -> String {
        let json = divi_oracle_tier_defaults_all()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the feature visibility for a sovereignty tier.
    ///
    /// `tierJSON` is a JSON SovereigntyTier (e.g. `"Steward"`).
    /// Returns JSON FeatureVisibility.
    public static func featureVisibility(tierJSON: String) throws -> String {
        guard let json = divi_oracle_feature_visibility(tierJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get feature visibility")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Workflow Helpers (stateless)

/// Stateless workflow validation and evaluation helpers.
///
/// These operate on JSON round-trips -- no persistent state.
/// For a managed collection of workflows, use `OracleWorkflowRegistry`.
public enum OracleWorkflow {

    /// Create and validate a workflow from JSON.
    ///
    /// Deserializes and re-serializes to normalize default fields.
    /// Returns JSON Workflow.
    public static func new(workflowJSON: String) throws -> String {
        guard let json = divi_oracle_workflow_new(workflowJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create workflow")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Check if a trigger matches an event.
    public static func triggerMatches(triggerJSON: String, eventJSON: String) throws -> Bool {
        let result = divi_oracle_trigger_matches(triggerJSON, eventJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to evaluate trigger match")
        }
        return result == 1
    }

    /// Evaluate a condition against a set of fields.
    ///
    /// `fieldsJSON` is a JSON `HashMap<String, String>`.
    public static func conditionEvaluate(conditionJSON: String, fieldsJSON: String) throws -> Bool {
        let result = divi_oracle_condition_evaluate(conditionJSON, fieldsJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to evaluate condition")
        }
        return result == 1
    }

    /// Check if a workflow should fire for the given event.
    public static func shouldFire(workflowJSON: String, eventJSON: String) throws -> Bool {
        let result = divi_oracle_workflow_should_fire(workflowJSON, eventJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to evaluate workflow should-fire")
        }
        return result == 1
    }
}

// MARK: - Hint Helpers

/// Static hint evaluation -- contextual guidance for onboarding and discovery.
public enum OracleHint {

    /// Create and validate a StaticHint from JSON.
    ///
    /// Deserializes and re-serializes to normalize. Returns JSON StaticHint.
    public static func newStaticHint(hintJSON: String) throws -> String {
        guard let json = divi_oracle_static_hint_new(hintJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create static hint")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Check if a static hint should be shown given a context.
    public static func shouldShow(hintJSON: String, contextJSON: String) throws -> Bool {
        let result = divi_oracle_static_hint_should_show(hintJSON, contextJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to evaluate hint visibility")
        }
        return result == 1
    }

    /// Create an empty HintContext.
    ///
    /// Returns JSON HintContext.
    public static func newContext() -> String {
        let json = divi_oracle_hint_context_new()!
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - Workflow Registry (stateful container)

/// A managed collection of workflows with consent tracking and audit logging.
///
/// Wraps the Rust `WorkflowRegistry` behind a Mutex via opaque pointer.
/// Thread-safe -- all mutations go through the lock.
///
/// ```swift
/// let registry = OracleWorkflowRegistry()
/// try registry.register(workflowJSON: myWorkflow)
/// try registry.grantConsent(id: "wf-1")
/// let matches = try registry.evaluate(eventJSON: event, timestamp: now)
/// ```
public final class OracleWorkflowRegistry: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_oracle_registry_new()!
    }

    deinit {
        divi_oracle_registry_free(ptr)
    }

    /// Register a workflow. Replaces any existing workflow with the same ID.
    public func register(workflowJSON: String) throws {
        let result = divi_oracle_registry_register(ptr, workflowJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register workflow")
        }
    }

    /// Unregister a workflow by ID.
    ///
    /// Returns JSON of the removed workflow, or nil if not found.
    public func unregister(id: String) -> String? {
        guard let json = divi_oracle_registry_unregister(ptr, id) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get a workflow by ID. Returns nil if not found.
    public func get(id: String) -> String? {
        guard let json = divi_oracle_registry_get(ptr, id) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Enable a workflow by ID.
    public func enable(id: String) throws {
        let result = divi_oracle_registry_enable(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to enable workflow '\(id)'")
        }
    }

    /// Disable a workflow by ID.
    public func disable(id: String) throws {
        let result = divi_oracle_registry_disable(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to disable workflow '\(id)'")
        }
    }

    /// Grant consent for a workflow by ID.
    public func grantConsent(id: String) throws {
        let result = divi_oracle_registry_grant_consent(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to grant consent for workflow '\(id)'")
        }
    }

    /// Revoke consent for a workflow by ID.
    public func revokeConsent(id: String) throws {
        let result = divi_oracle_registry_revoke_consent(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to revoke consent for workflow '\(id)'")
        }
    }

    /// List all registered workflows.
    ///
    /// Returns JSON array of Workflows.
    public func list() -> String {
        let json = divi_oracle_registry_list(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// List workflows for a specific actor.
    ///
    /// Returns JSON array of Workflows.
    public func listForActor(actor: String) -> String {
        let json = divi_oracle_registry_list_for_actor(ptr, actor)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Evaluate an event against all workflows in the registry.
    ///
    /// `timestamp` is a Unix timestamp in seconds.
    /// Returns JSON array of WorkflowMatch (fired actions).
    public func evaluate(eventJSON: String, timestamp: UInt64) throws -> String {
        guard let json = divi_oracle_registry_evaluate(ptr, eventJSON, timestamp) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to evaluate workflows")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// The number of registered workflows.
    public var count: Int {
        Int(divi_oracle_registry_count(ptr))
    }

    /// Export all workflows as JSON for persistence.
    ///
    /// Returns JSON array of Workflows.
    public func export() -> String {
        let json = divi_oracle_registry_export(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Import workflows from JSON (e.g. from persistence).
    ///
    /// Replaces existing workflows with the same IDs.
    public func importWorkflows(json: String) throws {
        let result = divi_oracle_registry_import(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to import workflows")
        }
    }

    /// Get the full audit log.
    ///
    /// Returns JSON array of AuditEntry.
    public func auditLog() -> String {
        let json = divi_oracle_registry_audit_log(ptr)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get audit entries for a specific workflow.
    ///
    /// Returns JSON array of AuditEntry.
    public func auditForWorkflow(id: String) -> String {
        let json = divi_oracle_registry_audit_for_workflow(ptr, id)!
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Clear the audit log.
    public func clearAudit() throws {
        let result = divi_oracle_registry_clear_audit(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to clear audit log")
        }
    }
}
