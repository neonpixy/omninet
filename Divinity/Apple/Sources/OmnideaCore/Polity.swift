import COmnideaFFI
import Foundation

// MARK: - PolityFoundation

/// Stateless access to the immutable constitutional foundation.
///
/// These are compile-time constants baked into the Covenant: the three axioms
/// (Dignity, Sovereignty, Consent), the rights that can never be revoked,
/// and the prohibitions that can never be lifted.
public enum PolityFoundation {

    /// The immutable rights as a JSON array of `RightCategory` strings.
    public static func immutableRights() -> String {
        let cstr = divi_polity_immutable_rights()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The absolute prohibitions as a JSON array of `ProhibitionType` strings.
    public static func immutableProhibitions() -> String {
        let cstr = divi_polity_immutable_prohibitions()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The three axioms as a JSON array of strings.
    public static func axioms() -> String {
        let cstr = divi_polity_axioms()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Whether a description would violate the immutable foundation.
    public static func wouldViolate(description: String) throws -> Bool {
        let result = divi_polity_would_violate(description)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check foundation violation")
        }
        return result == 1
    }

    /// Whether a right category is immutable.
    ///
    /// `categoryJSON` is a JSON `RightCategory` string (e.g. `"Dignity"`).
    public static func isRightImmutable(categoryJSON: String) throws -> Bool {
        let result = divi_polity_is_right_immutable(categoryJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check right immutability")
        }
        return result == 1
    }

    /// Whether a prohibition type is absolute.
    ///
    /// `prohibitionJSON` is a JSON `ProhibitionType` string (e.g. `"Surveillance"`).
    public static func isProhibitionAbsolute(prohibitionJSON: String) throws -> Bool {
        let result = divi_polity_is_prohibition_absolute(prohibitionJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check prohibition absoluteness")
        }
        return result == 1
    }
}

// MARK: - PolityRightsRegistry

/// Registry of rights in the constitutional framework.
///
/// Manages the full set of rights -- both immutable Covenant rights
/// and community-defined rights. Immutable rights cannot be removed.
///
/// ```swift
/// let rights = PolityRightsRegistry(withCovenant: true)
/// let id = try rights.register(rightJSON: myRight)
/// let all = rights.all()
/// ```
public final class PolityRightsRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty rights registry.
    public init() {
        ptr = divi_polity_rights_new()!
    }

    /// Create a rights registry, optionally pre-populated with Covenant rights.
    public init(withCovenant: Bool) {
        if withCovenant {
            ptr = divi_polity_rights_new_with_covenant()!
        } else {
            ptr = divi_polity_rights_new()!
        }
    }

    deinit {
        divi_polity_rights_free(ptr)
    }

    /// Register a new right. Returns the UUID of the registered right.
    ///
    /// `rightJSON` is a JSON `Right` object.
    public func register(rightJSON: String) throws -> String {
        guard let cstr = divi_polity_rights_register(ptr, rightJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register right")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a right by UUID. Returns JSON `Right`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_rights_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get rights by category. Returns JSON array of `Right`.
    ///
    /// `categoryJSON` is a JSON `RightCategory` (e.g. `"Dignity"`).
    public func byCategory(categoryJSON: String) throws -> String {
        guard let cstr = divi_polity_rights_by_category(ptr, categoryJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get rights by category")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Find a right by name (case-insensitive). Returns JSON `Right`, or nil if not found.
    public func findByName(_ name: String) -> String? {
        guard let cstr = divi_polity_rights_find_by_name(ptr, name) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All rights in the registry as a JSON array.
    public func all() -> String {
        let cstr = divi_polity_rights_all(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All immutable rights as a JSON array.
    public func immutable() -> String {
        let cstr = divi_polity_rights_immutable(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Remove a right by UUID. Throws if the right is immutable.
    public func remove(id: String) throws {
        let result = divi_polity_rights_remove(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove right '\(id)'")
        }
    }

    /// The number of rights in the registry.
    public var count: Int {
        Int(divi_polity_rights_count(ptr))
    }
}

// MARK: - PolityDutiesRegistry

/// Registry of duties in the constitutional framework.
///
/// Mirrors `PolityRightsRegistry` but for duties -- obligations that
/// participants owe to each other and to the community.
///
/// ```swift
/// let duties = PolityDutiesRegistry(withCovenant: true)
/// let absolute = duties.absolute()
/// ```
public final class PolityDutiesRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty duties registry.
    public init() {
        ptr = divi_polity_duties_new()!
    }

    /// Create a duties registry, optionally pre-populated with Covenant duties.
    public init(withCovenant: Bool) {
        if withCovenant {
            ptr = divi_polity_duties_new_with_covenant()!
        } else {
            ptr = divi_polity_duties_new()!
        }
    }

    deinit {
        divi_polity_duties_free(ptr)
    }

    /// Register a new duty. Returns the UUID of the registered duty.
    ///
    /// `dutyJSON` is a JSON `Duty` object.
    public func register(dutyJSON: String) throws -> String {
        guard let cstr = divi_polity_duties_register(ptr, dutyJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register duty")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a duty by UUID. Returns JSON `Duty`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_duties_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get duties by category. Returns JSON array of `Duty`.
    ///
    /// `categoryJSON` is a JSON `DutyCategory` (e.g. `"Steward"`).
    public func byCategory(categoryJSON: String) throws -> String {
        guard let cstr = divi_polity_duties_by_category(ptr, categoryJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get duties by category")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Find a duty by name (case-insensitive). Returns JSON `Duty`, or nil if not found.
    public func findByName(_ name: String) -> String? {
        guard let cstr = divi_polity_duties_find_by_name(ptr, name) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All duties in the registry as a JSON array.
    public func all() -> String {
        let cstr = divi_polity_duties_all(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All absolute (non-removable) duties as a JSON array.
    public func absolute() -> String {
        let cstr = divi_polity_duties_absolute(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Remove a duty by UUID. Throws if the duty is immutable.
    public func remove(id: String) throws {
        let result = divi_polity_duties_remove(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove duty '\(id)'")
        }
    }

    /// The number of duties in the registry.
    public var count: Int {
        Int(divi_polity_duties_count(ptr))
    }
}

// MARK: - PolityProtectionsRegistry

/// Registry of protections against prohibited actions.
///
/// Each protection maps a prohibited action type to an enforcement rule.
/// Absolute protections cannot be removed.
///
/// ```swift
/// let protections = PolityProtectionsRegistry(withCovenant: true)
/// let violations = try protections.checkViolation(actionJSON: action)
/// ```
public final class PolityProtectionsRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create an empty protections registry.
    public init() {
        ptr = divi_polity_protections_new()!
    }

    /// Create a protections registry, optionally pre-populated with Covenant protections.
    public init(withCovenant: Bool) {
        if withCovenant {
            ptr = divi_polity_protections_new_with_covenant()!
        } else {
            ptr = divi_polity_protections_new()!
        }
    }

    deinit {
        divi_polity_protections_free(ptr)
    }

    /// Register a new protection. Returns the UUID of the registered protection.
    ///
    /// `protectionJSON` is a JSON `Protection` object.
    public func register(protectionJSON: String) throws -> String {
        guard let cstr = divi_polity_protections_register(ptr, protectionJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register protection")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a protection by UUID. Returns JSON `Protection`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_protections_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get protections by prohibition type. Returns JSON array of `Protection`.
    ///
    /// `typeJSON` is a JSON `ProhibitionType` (e.g. `"Surveillance"`).
    public func byType(typeJSON: String) throws -> String {
        guard let cstr = divi_polity_protections_by_type(ptr, typeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get protections by type")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Find a protection by name (case-insensitive). Returns JSON `Protection`, or nil.
    public func findByName(_ name: String) -> String? {
        guard let cstr = divi_polity_protections_find_by_name(ptr, name) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All protections in the registry as a JSON array.
    public func all() -> String {
        let cstr = divi_polity_protections_all(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All absolute (non-removable) protections as a JSON array.
    public func absolute() -> String {
        let cstr = divi_polity_protections_absolute(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Check whether an action violates any protection.
    ///
    /// Returns a JSON array of `Protection` objects that the action violates.
    /// `actionJSON` is a JSON `ActionDescription`.
    public func checkViolation(actionJSON: String) throws -> String {
        guard let cstr = divi_polity_protections_check_violation(ptr, actionJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check violation")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Remove a protection by UUID. Throws if the protection is absolute.
    public func remove(id: String) throws {
        let result = divi_polity_protections_remove(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove protection '\(id)'")
        }
    }

    /// The number of protections in the registry.
    public var count: Int {
        Int(divi_polity_protections_count(ptr))
    }
}

// MARK: - PolityReviewer

/// Constitutional reviewer -- checks actions against rights and protections.
///
/// Stateless: borrows from a `PolityRightsRegistry` and a `PolityProtectionsRegistry`
/// for each call. The Rust side constructs a temporary `ConstitutionalReviewer` with
/// consistent lock ordering (rights first, then protections).
///
/// ```swift
/// let rights = PolityRightsRegistry(withCovenant: true)
/// let protections = PolityProtectionsRegistry(withCovenant: true)
/// let review = try PolityReviewer.review(rights: rights, protections: protections, actionJSON: action)
/// ```
public enum PolityReviewer {

    /// Review an action against the Covenant.
    ///
    /// Returns a JSON `ConstitutionalReview` describing the outcome.
    public static func review(
        rights: PolityRightsRegistry,
        protections: PolityProtectionsRegistry,
        actionJSON: String
    ) throws -> String {
        guard let cstr = divi_polity_review(rights.ptr, protections.ptr, actionJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to review action")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Quick check: does an action violate any absolute prohibition?
    public static func isAbsolutelyProhibited(
        rights: PolityRightsRegistry,
        protections: PolityProtectionsRegistry,
        actionJSON: String
    ) throws -> Bool {
        let result = divi_polity_is_absolutely_prohibited(rights.ptr, protections.ptr, actionJSON)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check absolute prohibition")
        }
        return result == 1
    }

    /// Convert a review to a formal Breach record, if the review found a violation.
    ///
    /// Returns JSON `Breach` if the review contained a breach, or nil if the review was clean.
    public static func reviewToBreach(
        rights: PolityRightsRegistry,
        protections: PolityProtectionsRegistry,
        reviewJSON: String
    ) throws -> String? {
        guard let cstr = divi_polity_review_to_breach(rights.ptr, protections.ptr, reviewJSON) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - PolityBreachRegistry

/// Registry of constitutional breaches.
///
/// Records, tracks, and queries breaches of the Covenant. Supports
/// filtering by actor, severity, active status, and foundational breaches.
///
/// ```swift
/// let breaches = PolityBreachRegistry()
/// let id = try breaches.record(breachJSON: breach)
/// try breaches.updateStatus(id: id, statusJSON: "\"Investigating\"")
/// ```
public final class PolityBreachRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new breach registry.
    public init() {
        ptr = divi_polity_breach_new()!
    }

    deinit {
        divi_polity_breach_free(ptr)
    }

    /// Record a breach. Returns the UUID of the recorded breach.
    ///
    /// `breachJSON` is a JSON `Breach` object.
    public func record(breachJSON: String) throws -> String {
        guard let cstr = divi_polity_breach_record(ptr, breachJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record breach")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a breach by UUID. Returns JSON `Breach`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_breach_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Update a breach's status.
    ///
    /// `statusJSON` is a JSON `BreachStatus` (e.g. `"Investigating"`).
    public func updateStatus(id: String, statusJSON: String) throws {
        let result = divi_polity_breach_update_status(ptr, id, statusJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update breach status '\(id)'")
        }
    }

    /// Get all breaches by a specific actor. Returns JSON array of `Breach`.
    public func byActor(_ actor: String) -> String {
        let cstr = divi_polity_breach_by_actor(ptr, actor)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get breaches by severity. Returns JSON array of `Breach`.
    ///
    /// `severityJSON` is a JSON `BreachSeverity` (e.g. `"Grave"`).
    public func bySeverity(severityJSON: String) throws -> String {
        guard let cstr = divi_polity_breach_by_severity(ptr, severityJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get breaches by severity")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All active breaches (not resolved or dismissed) as a JSON array.
    public func active() -> String {
        let cstr = divi_polity_breach_active(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// All foundational breaches (involving immutable foundations) as a JSON array.
    public func foundational() -> String {
        let cstr = divi_polity_breach_foundational(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of breaches in the registry.
    public var count: Int {
        Int(divi_polity_breach_count(ptr))
    }
}

// MARK: - PolityAmendment

/// Pure JSON transforms for the amendment lifecycle.
///
/// Amendments are the mechanism for evolving the Covenant's non-immutable parts.
/// Each function takes an amendment JSON, applies a state transition, and returns
/// the modified amendment JSON. The immutable foundation cannot be amended.
///
/// Lifecycle: Proposed -> Deliberation -> Ratification -> Enacted/Rejected/Nullified
///
/// ```swift
/// let amendment = try PolityAmendment.new(triggerJSON: "\"Contradiction\"", ...)
/// let deliberating = try PolityAmendment.beginDeliberation(amendment)
/// let ratifying = try PolityAmendment.beginRatification(deliberating)
/// let enacted = try PolityAmendment.enact(ratifying)
/// ```
public enum PolityAmendment {

    /// Create a new amendment.
    ///
    /// Returns JSON `Amendment`, or throws if the description would violate
    /// the immutable foundation.
    ///
    /// - Parameters:
    ///   - triggerJSON: JSON `AmendmentTrigger` (e.g. `"Contradiction"`).
    ///   - title: Short title for the amendment.
    ///   - description: Full description of the proposed change.
    ///   - proposer: The proposer's identifier.
    public static func new(
        triggerJSON: String,
        title: String,
        description: String,
        proposer: String
    ) throws -> String {
        guard let cstr = divi_polity_amendment_new(triggerJSON, title, description, proposer) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create amendment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Move an amendment from Proposed to Deliberation.
    public static func beginDeliberation(_ amendmentJSON: String) throws -> String {
        guard let cstr = divi_polity_amendment_begin_deliberation(amendmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to begin deliberation")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Move an amendment from Deliberation to Ratification.
    public static func beginRatification(_ amendmentJSON: String) throws -> String {
        guard let cstr = divi_polity_amendment_begin_ratification(amendmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to begin ratification")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Add support from a participant.
    public static func addSupport(_ amendmentJSON: String, supporter: String) throws -> String {
        guard let cstr = divi_polity_amendment_add_support(amendmentJSON, supporter) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add support")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Add objection from a participant.
    public static func addObjection(_ amendmentJSON: String, objector: String) throws -> String {
        guard let cstr = divi_polity_amendment_add_objection(amendmentJSON, objector) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add objection")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Update the support ratio (0.0 to 1.0).
    public static func updateSupport(_ amendmentJSON: String, ratio: Double) throws -> String {
        guard let cstr = divi_polity_amendment_update_support(amendmentJSON, ratio) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update support ratio")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Attempt to enact an amendment. Returns the enacted amendment JSON,
    /// or throws if the threshold is not met or the transition is invalid.
    public static func enact(_ amendmentJSON: String) throws -> String {
        guard let cstr = divi_polity_amendment_enact(amendmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to enact amendment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Reject an amendment.
    public static func reject(_ amendmentJSON: String) throws -> String {
        guard let cstr = divi_polity_amendment_reject(amendmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to reject amendment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Nullify an amendment (contradicts immutable foundations).
    public static func nullify(_ amendmentJSON: String) throws -> String {
        guard let cstr = divi_polity_amendment_nullify(amendmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to nullify amendment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - PolityEnactment

/// Pure JSON transforms for enactment lifecycle (swearing the oath).
///
/// An enactment is a person or community formally affirming the Covenant.
/// Lifecycle: Active -> Suspended -> Reactivated, or Active -> Withdrawn.
public enum PolityEnactment {

    /// Create a new enactment.
    ///
    /// - Parameters:
    ///   - enactor: The enactor's identifier.
    ///   - enactorTypeJSON: JSON `EnactorType` (e.g. `"Person"`).
    ///   - affirmation: The oath text.
    public static func new(
        enactor: String,
        enactorTypeJSON: String,
        affirmation: String
    ) throws -> String {
        guard let cstr = divi_polity_enactment_new(enactor, enactorTypeJSON, affirmation) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create enactment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Suspend an enactment.
    public static func suspend(_ enactmentJSON: String) throws -> String {
        guard let cstr = divi_polity_enactment_suspend(enactmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to suspend enactment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Reactivate a suspended enactment.
    public static func reactivate(_ enactmentJSON: String) throws -> String {
        guard let cstr = divi_polity_enactment_reactivate(enactmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to reactivate enactment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Withdraw from the Covenant.
    public static func withdraw(_ enactmentJSON: String) throws -> String {
        guard let cstr = divi_polity_enactment_withdraw(enactmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to withdraw enactment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The default oath of enactment.
    public static func defaultOath() -> String {
        let cstr = divi_polity_default_oath()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - PolityEnactmentRegistry

/// Registry of Covenant enactments -- who has sworn the oath.
///
/// Enforces uniqueness: one active enactment per enactor at a time.
///
/// ```swift
/// let registry = PolityEnactmentRegistry()
/// let oath = PolityEnactment.defaultOath()
/// let enactment = try PolityEnactment.new(enactor: pubkey, enactorTypeJSON: "\"Person\"", affirmation: oath)
/// let id = try registry.record(enactmentJSON: enactment)
/// ```
public final class PolityEnactmentRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new enactment registry.
    public init() {
        ptr = divi_polity_enactment_registry_new()!
    }

    deinit {
        divi_polity_enactment_registry_free(ptr)
    }

    /// Record an enactment. Returns the UUID of the recorded enactment.
    ///
    /// Throws if the enactor already has an active enactment.
    public func record(enactmentJSON: String) throws -> String {
        guard let cstr = divi_polity_enactment_registry_record(ptr, enactmentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record enactment")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get an enactment by UUID. Returns JSON `Enactment`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_enactment_registry_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Whether the given enactor has an active enactment.
    public func isEnacted(enactor: String) throws -> Bool {
        let result = divi_polity_enactment_registry_is_enacted(ptr, enactor)
        if result == -1 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to check enactment status")
        }
        return result == 1
    }

    /// All active enactments as a JSON array.
    public func active() -> String {
        let cstr = divi_polity_enactment_registry_active(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of enactments in the registry.
    public var count: Int {
        Int(divi_polity_enactment_registry_count(ptr))
    }
}

// MARK: - PolityConsent

/// Pure JSON transforms for the consent lifecycle.
///
/// Consent is voluntary, informed, continuous, and revocable.
/// Every data flow requires explicit consent from the grantor.
public enum PolityConsent {

    /// Create a new consent record.
    ///
    /// - Parameters:
    ///   - grantor: The grantor's identifier (who gives consent).
    ///   - recipient: The recipient's identifier (who receives consent).
    ///   - scopeJSON: JSON `ConsentScope` defining what is consented to.
    public static func new(
        grantor: String,
        recipient: String,
        scopeJSON: String
    ) throws -> String {
        guard let cstr = divi_polity_consent_new(grantor, recipient, scopeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create consent record")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Revoke a consent record.
    ///
    /// Returns the modified consent JSON with revocation timestamp and reason.
    public static func revoke(_ consentJSON: String, reason: String) throws -> String {
        guard let cstr = divi_polity_consent_revoke(consentJSON, reason) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to revoke consent")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - PolityConsentRegistry

/// Registry of consent records.
///
/// Tracks all consent grants and revocations. Supports validation
/// of whether a specific scope is currently consented between two parties.
///
/// ```swift
/// let registry = PolityConsentRegistry()
/// let consent = try PolityConsent.new(grantor: "alice", recipient: "bob", scopeJSON: scope)
/// let id = try registry.record(consentJSON: consent)
/// let validation = try registry.validate(grantor: "alice", recipient: "bob", scopeJSON: scope)
/// ```
public final class PolityConsentRegistry: @unchecked Sendable {
    let ptr: OpaquePointer

    /// Create a new consent registry.
    public init() {
        ptr = divi_polity_consent_registry_new()!
    }

    deinit {
        divi_polity_consent_registry_free(ptr)
    }

    /// Record a consent in the registry. Returns the UUID of the recorded consent.
    ///
    /// `consentJSON` is a JSON `ConsentRecord`.
    public func record(consentJSON: String) throws -> String {
        guard let cstr = divi_polity_consent_registry_record(ptr, consentJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to record consent")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a consent record by UUID. Returns JSON `ConsentRecord`, or nil if not found.
    public func get(id: String) throws -> String? {
        guard let cstr = divi_polity_consent_registry_get(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get all consent records by grantor. Returns JSON array of `ConsentRecord`.
    public func byGrantor(_ grantor: String) -> String {
        let cstr = divi_polity_consent_registry_by_grantor(ptr, grantor)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Revoke a consent record in the registry by UUID.
    public func revoke(id: String, reason: String) throws {
        let result = divi_polity_consent_registry_revoke(ptr, id, reason)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to revoke consent '\(id)'")
        }
    }

    /// All active (non-revoked) consent records as a JSON array.
    public func active() -> String {
        let cstr = divi_polity_consent_registry_active(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Validate consent for a given scope between grantor and recipient.
    ///
    /// Returns JSON `ConsentValidation` describing whether the scope is covered.
    public func validate(grantor: String, recipient: String, scopeJSON: String) throws -> String {
        guard let cstr = divi_polity_consent_registry_validate(ptr, grantor, recipient, scopeJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to validate consent")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of consent records in the registry.
    public var count: Int {
        Int(divi_polity_consent_registry_count(ptr))
    }
}
