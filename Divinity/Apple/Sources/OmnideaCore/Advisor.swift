import COmnideaFFI
import Foundation
import os

private let logger = Logger(subsystem: "co.omnidea", category: "Advisor")

// MARK: - AdvisorLoop

/// The AI cognitive loop — the brain stem of the Advisor.
///
/// AdvisorLoop manages the tick-driven cognitive cycle: accumulating thoughts,
/// building expression pressure, and emitting actions when thresholds are met.
/// Energy and novelty levels modulate the loop's behavior. Conversations create
/// focused sessions within the loop.
///
/// ```swift
/// let loop = AdvisorLoop(sessionId: sessionUUID)
/// let actions = loop.tick(elapsedMs: 2000)
/// // Parse JSON actions array for CognitiveAction events
/// ```
public final class AdvisorLoop: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new cognitive loop.
    ///
    /// - Parameters:
    ///   - configJSON: JSON `AdvisorConfig`, or nil for default configuration.
    ///   - sessionId: UUID string for the home session.
    public init?(configJSON: String? = nil, sessionId: String) {
        guard let p = divi_advisor_loop_new(configJSON, sessionId) else {
            logger.error("Failed to create AdvisorLoop")
            return nil
        }
        ptr = p
    }

    deinit {
        divi_advisor_loop_free(ptr)
    }

    /// Advance the loop by one tick. Returns JSON array of `CognitiveAction`.
    ///
    /// The tick drives the entire cognitive cycle: decay, pressure evaluation,
    /// and action emission. Call this on a regular cadence (e.g., every 1-2 seconds).
    public func tick(elapsedMs: UInt64) -> String? {
        guard let json = divi_advisor_loop_tick(ptr, elapsedMs) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Feed an LLM generation result back into the loop.
    ///
    /// After sending a generation request to a provider, pass the result back
    /// here so the loop can process it and emit follow-up actions.
    ///
    /// - Parameter resultJSON: JSON `GenerationResult` from the provider.
    /// - Returns: JSON array of `CognitiveAction`, or nil on error.
    public func receiveGeneration(resultJSON: String) -> String? {
        guard let json = divi_advisor_loop_receive_generation(ptr, resultJSON) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Apply a state command to the loop (e.g., change mode).
    ///
    /// - Parameter commandJSON: JSON `StateCommand`.
    /// - Returns: JSON array of `CognitiveAction`, or nil on error.
    public func applyCommand(_ commandJSON: String) -> String? {
        guard let json = divi_advisor_loop_apply_command(ptr, commandJSON) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set the energy level (0.0-1.0). Higher energy = more active cognition.
    public func setEnergy(_ energy: Double) {
        divi_advisor_loop_set_energy(ptr, energy)
    }

    /// Set the novelty level (0.0-1.0). Higher novelty = more exploratory.
    public func setNovelty(_ novelty: Double) {
        divi_advisor_loop_set_novelty(ptr, novelty)
    }

    /// Notify the loop that a conversation has started.
    public func beginConversation() {
        divi_advisor_loop_begin_conversation(ptr)
    }

    /// Notify the loop that a conversation has ended.
    public func endConversation() {
        divi_advisor_loop_end_conversation(ptr)
    }

    /// Get a pressure snapshot. Returns JSON `PressureSnapshot`.
    public var pressureSnapshot: String? {
        guard let json = divi_advisor_loop_pressure_snapshot(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the current cognitive mode. Returns JSON `CognitiveMode`.
    public var mode: String? {
        guard let json = divi_advisor_loop_mode(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get the current expression consent. Returns JSON `ExpressionConsent`.
    public var consent: String? {
        guard let json = divi_advisor_loop_consent(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Set expression consent on the loop.
    ///
    /// - Parameter consentJSON: JSON `ExpressionConsent`.
    public func setConsent(_ consentJSON: String) throws {
        let result = divi_advisor_loop_set_consent(ptr, consentJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set consent")
        }
    }
}

// MARK: - AdvisorStore

/// In-memory cognitive state — thoughts, sessions, memories, and synapses.
///
/// The store is the Advisor's working memory. Thoughts are individual cognitive
/// events. Sessions group thoughts into conversations. Memories persist beyond
/// sessions. Synapses connect thoughts into associative networks.
///
/// ```swift
/// let store = AdvisorStore(clipboardMax: 100)
/// try store.saveThought(thoughtJSON)
/// let memories = store.searchMemories(query: "design", maxResults: 10)
/// ```
public final class AdvisorStore: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new cognitive store.
    ///
    /// - Parameter clipboardMax: Maximum number of clipboard entries to retain.
    public init(clipboardMax: Int = 100) {
        ptr = divi_advisor_store_new(UInt(clipboardMax))!
    }

    deinit {
        divi_advisor_store_free(ptr)
    }

    // MARK: Thoughts

    /// Save a thought to the store.
    ///
    /// - Parameter thoughtJSON: JSON `Thought`.
    public func saveThought(_ thoughtJSON: String) throws {
        let result = divi_advisor_store_save_thought(ptr, thoughtJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save thought")
        }
    }

    /// Get a thought by ID. Returns JSON `Thought`, or nil if not found.
    ///
    /// - Parameter id: UUID string.
    public func getThought(id: String) throws -> String? {
        guard let json = divi_advisor_store_get_thought(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all thoughts for a session. Returns JSON array of `Thought`.
    ///
    /// - Parameter sessionId: UUID string.
    public func thoughtsForSession(_ sessionId: String) -> String? {
        guard let json = divi_advisor_store_thoughts_for_session(ptr, sessionId) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Delete a thought by ID. Returns true if the thought was found and deleted.
    ///
    /// - Parameter id: UUID string.
    public func deleteThought(id: String) -> Bool {
        divi_advisor_store_delete_thought(ptr, id)
    }

    /// Number of thoughts in the store.
    public var thoughtCount: Int {
        Int(divi_advisor_store_thought_count(ptr))
    }

    // MARK: Sessions

    /// Save a session to the store.
    ///
    /// - Parameter sessionJSON: JSON `Session`.
    public func saveSession(_ sessionJSON: String) throws {
        let result = divi_advisor_store_save_session(ptr, sessionJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save session")
        }
    }

    /// Get a session by ID. Returns JSON `Session`, or nil if not found.
    ///
    /// - Parameter id: UUID string.
    public func getSession(id: String) throws -> String? {
        guard let json = divi_advisor_store_get_session(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all active (non-archived) sessions. Returns JSON array of `Session`.
    public func activeSessions() -> String? {
        guard let json = divi_advisor_store_active_sessions(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Number of sessions in the store.
    public var sessionCount: Int {
        Int(divi_advisor_store_session_count(ptr))
    }

    // MARK: Memories

    /// Save a memory to the store.
    ///
    /// - Parameter memoryJSON: JSON `Memory`.
    public func saveMemory(_ memoryJSON: String) throws {
        let result = divi_advisor_store_save_memory(ptr, memoryJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save memory")
        }
    }

    /// Search memories by keyword. Returns JSON array of `MemoryResult`.
    ///
    /// - Parameters:
    ///   - query: Search keyword.
    ///   - maxResults: Maximum number of results to return.
    public func searchMemories(query: String, maxResults: Int) -> String? {
        guard let json = divi_advisor_store_search_memories(ptr, query, UInt(maxResults)) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Number of memories in the store.
    public var memoryCount: Int {
        Int(divi_advisor_store_memory_count(ptr))
    }

    // MARK: Synapses

    /// Save a synapse (associative link between thoughts) to the store.
    ///
    /// - Parameter synapseJSON: JSON `Synapse`.
    public func saveSynapse(_ synapseJSON: String) throws {
        let result = divi_advisor_store_save_synapse(ptr, synapseJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save synapse")
        }
    }

    /// Prune synapses below a minimum strength threshold.
    ///
    /// - Parameter minStrength: Minimum strength (0.0-1.0). Synapses below this are removed.
    /// - Returns: Number of synapses pruned.
    @discardableResult
    public func pruneWeakSynapses(minStrength: Double) -> Int {
        Int(divi_advisor_store_prune_weak_synapses(ptr, minStrength))
    }

    /// Number of synapses in the store.
    public var synapseCount: Int {
        Int(divi_advisor_store_synapse_count(ptr))
    }
}

// MARK: - AdvisorRouter

/// Multi-provider selection and routing for AI backends.
///
/// The router maintains a registry of cognitive providers (Claude API, Apple
/// Intelligence, Ollama, MLX, etc.) and selects the best one for a given request
/// based on capability requirements, user preferences, and security tier.
///
/// Provider registration uses a callback-based pattern: you supply a C function
/// pointer for querying provider status, which the router calls when selecting.
///
/// ```swift
/// let router = AdvisorRouter()
/// // Register providers via registerProvider(...)
/// if let best = router.select(requiredCapabilities: 0x03) {
///     // Use the selected provider
/// }
/// ```
public final class AdvisorRouter: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new provider router with an empty registry.
    public init() {
        ptr = divi_advisor_router_new()!
    }

    deinit {
        divi_advisor_router_free(ptr)
    }

    /// Register a cognitive provider.
    ///
    /// - Parameters:
    ///   - id: Unique provider identifier.
    ///   - name: Human-readable display name.
    ///   - capabilitiesBitflags: Raw u32 bitflags of `ProviderCapabilities`.
    ///   - isCloud: Whether the provider requires cloud access.
    ///   - statusFn: C callback that returns JSON `ProviderStatus`.
    ///   - context: Opaque pointer passed to the status callback. Must remain valid
    ///     for the lifetime of the registration.
    public func registerProvider(
        id: String,
        name: String,
        capabilitiesBitflags: UInt32,
        isCloud: Bool,
        statusFn: @escaping @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?,
        context: UnsafeMutableRawPointer?
    ) throws {
        let result = divi_advisor_router_register_provider(
            ptr, id, name, capabilitiesBitflags, isCloud, statusFn, context
        )
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register provider '\(id)'")
        }
    }

    /// Unregister a provider by ID.
    ///
    /// - Parameter id: The provider's unique identifier.
    public func unregister(id: String) throws {
        let result = divi_advisor_router_unregister(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to unregister provider '\(id)'")
        }
    }

    /// Set provider preferences (e.g., prefer local over cloud).
    ///
    /// - Parameter preferencesJSON: JSON `ProviderPreferences`.
    public func setPreferences(_ preferencesJSON: String) throws {
        let result = divi_advisor_router_set_preferences(ptr, preferencesJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set provider preferences")
        }
    }

    /// Set the security tier (controls which providers are eligible).
    ///
    /// - Parameter tierJSON: JSON `SecurityTier`.
    public func setSecurityTier(_ tierJSON: String) throws {
        let result = divi_advisor_router_set_security_tier(ptr, tierJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set security tier")
        }
    }

    /// Select the best provider for a request. Returns JSON `ProviderInfo`, or nil
    /// if no provider satisfies the requirements.
    ///
    /// - Parameter requiredCapabilities: Raw u32 bitflags of required `ProviderCapabilities`.
    public func select(requiredCapabilities: UInt32) throws -> String? {
        guard let json = divi_advisor_router_select(ptr, requiredCapabilities) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get info for all registered providers. Returns JSON array of `ProviderInfo`.
    public func providerInfo() -> String? {
        guard let json = divi_advisor_router_provider_info(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - AdvisorSkillRegistry

/// Registry of skills (tools/functions) the Advisor can invoke.
///
/// Skills represent capabilities like web search, file operations, or code
/// execution. The registry supports registration, lookup by ID, search by
/// keyword, and enumeration.
///
/// ```swift
/// let skills = AdvisorSkillRegistry()
/// try skills.register(skillJSON: webSearchSkill)
/// let all = skills.available()
/// print("Registered \(skills.count) skills")
/// ```
public final class AdvisorSkillRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new empty skill registry.
    public init() {
        ptr = divi_advisor_skills_new()!
    }

    deinit {
        divi_advisor_skills_free(ptr)
    }

    /// Register a skill.
    ///
    /// - Parameter skillJSON: JSON `SkillDefinition`.
    public func register(skillJSON: String) throws {
        let result = divi_advisor_skills_register(ptr, skillJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register skill")
        }
    }

    /// Unregister a skill by ID.
    ///
    /// - Parameter id: The skill's unique identifier.
    public func unregister(id: String) throws {
        let result = divi_advisor_skills_unregister(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to unregister skill '\(id)'")
        }
    }

    /// Get a skill by ID. Returns JSON `SkillDefinition`, or nil if not found.
    ///
    /// - Parameter id: The skill's unique identifier.
    public func get(id: String) throws -> String? {
        guard let json = divi_advisor_skills_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Search skills by name or description keyword. Returns JSON array of `SkillDefinition`.
    ///
    /// - Parameter query: Search keyword.
    public func search(query: String) -> String? {
        guard let json = divi_advisor_skills_search(ptr, query) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Get all registered skills. Returns JSON array of `SkillDefinition`.
    public func available() -> String? {
        guard let json = divi_advisor_skills_all(ptr) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Number of registered skills.
    public var count: Int {
        Int(divi_advisor_skills_count(ptr))
    }
}

// MARK: - AdvisorThought

/// Factory methods for creating and inspecting Thought values.
///
/// Thoughts are the atomic unit of cognition — individual observations,
/// insights, or responses that flow through the cognitive loop.
public enum AdvisorThought {

    /// Create a new thought. Returns JSON `Thought`.
    ///
    /// - Parameters:
    ///   - sessionId: UUID string of the owning session.
    ///   - content: The thought's text content.
    ///   - sourceJSON: JSON `ThoughtSource` (e.g., `"Autonomous"`, `{"User":"prompt"}`).
    public static func new(sessionId: String, content: String, sourceJSON: String) -> String? {
        guard let json = divi_advisor_thought_new(sessionId, content, sourceJSON) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - AdvisorSession

/// Factory methods for creating Session values.
///
/// Sessions group thoughts into coherent conversations. The home session is
/// the default ambient context; user sessions are explicitly created interactions.
public enum AdvisorSession {

    /// Create a home session (ambient, always-on context). Returns JSON `Session`.
    public static func home() -> String? {
        guard let json = divi_advisor_session_home() else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create a user session with a title. Returns JSON `Session`.
    ///
    /// - Parameter title: Display title for the session.
    public static func user(title: String) -> String? {
        guard let json = divi_advisor_session_user(title) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - AdvisorConfig

/// Factory methods for AdvisorConfig presets.
///
/// Presets tune the cognitive loop's thresholds, decay rates, and expression
/// pressure for different use cases.
public enum AdvisorConfig {

    /// Default configuration — balanced between contemplative and responsive.
    /// Returns JSON `AdvisorConfig`.
    public static func `default`() -> String? {
        guard let json = divi_advisor_config_default() else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Contemplative preset — slower decay, higher expression threshold.
    /// The Advisor thinks longer before speaking. Returns JSON `AdvisorConfig`.
    public static func contemplative() -> String? {
        guard let json = divi_advisor_config_contemplative() else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Responsive preset — faster decay, lower expression threshold.
    /// The Advisor responds more quickly. Returns JSON `AdvisorConfig`.
    public static func responsive() -> String? {
        guard let json = divi_advisor_config_responsive() else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - AdvisorSponsorship

/// Factory methods for AI companion sponsorship bonds.
///
/// Every AI companion in Omnidea is sponsored by a human. The bond ties the
/// companion's identity to a responsible human who vouches for its behavior.
/// One companion per sponsor, bound by the Covenant like everyone else.
public enum AdvisorSponsorship {

    /// Create a new sponsorship bond. Returns JSON `SponsorshipBond`.
    ///
    /// - Parameters:
    ///   - sponsor: The sponsor's public key (crownId).
    ///   - companionId: UUID string for the companion identity.
    public static func newBond(sponsor: String, companionId: String) -> String? {
        guard let json = divi_advisor_sponsorship_bond_new(sponsor, companionId) else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}

// MARK: - AdvisorConsent

/// Factory methods for expression consent.
///
/// Expression consent controls whether and how the Advisor may speak.
/// Consent is continuous and revocable — the human always decides.
public enum AdvisorConsent {

    /// Get the default expression consent (granted, normal level).
    /// Returns JSON `ExpressionConsent`.
    public static func `default`() -> String? {
        guard let json = divi_advisor_expression_consent_default() else {
            return nil
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }
}
