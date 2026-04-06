import COmnideaFFI
import Foundation
import os

private let logger = Logger(subsystem: "co.omnidea", category: "Ideas")

// MARK: - IdeaPackage

/// Swift wrapper around the Rust IdeaPackage (the .idea universal content format).
///
/// An IdeaPackage represents a single .idea directory on disk. It contains a
/// header (identity + metadata), a tree of digits (content nodes), and optional
/// attachments like bonds, book (authority), tree (provenance), cool (currency),
/// redemption, and position.
///
/// ```swift
/// let pkg = try IdeaPackage(loadFrom: "/path/to/my.idea")
/// let header = pkg.header()
/// let root = pkg.rootDigit()
/// let count = pkg.digitCount
/// try pkg.save()
/// ```
public final class IdeaPackage: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Create a new IdeaPackage in memory.
    ///
    /// - Parameters:
    ///   - path: Filesystem path for the .idea directory.
    ///   - headerJSON: A JSON-encoded Header.
    ///   - rootDigitJSON: A JSON-encoded Digit for the root node.
    public init(path: String, headerJSON: String, rootDigitJSON: String) throws {
        guard let p = divi_ideas_package_new(path, headerJSON, rootDigitJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create IdeaPackage")
        }
        self.ptr = p
    }

    /// Load an IdeaPackage from a .idea directory on disk.
    ///
    /// - Parameter path: Path to the .idea directory.
    public init(loadFrom path: String) throws {
        guard let p = divi_ideas_package_load(path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to load IdeaPackage from '\(path)'")
        }
        self.ptr = p
    }

    deinit {
        divi_ideas_package_free(ptr)
    }

    /// Save the package to disk.
    public func save() throws {
        let result = divi_ideas_package_save(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save IdeaPackage")
        }
    }

    /// Get the package header as a JSON string.
    public func header() -> String {
        let cstr = divi_ideas_package_header(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the root digit as a JSON string, or nil if not found.
    public func rootDigit() -> String? {
        guard let cstr = divi_ideas_package_root_digit(ptr) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get a digit by UUID string as JSON, or nil if not found.
    public func digit(id: String) throws -> String? {
        guard let cstr = divi_ideas_package_digit(ptr, id) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of digits in the package.
    public var digitCount: Int {
        Int(divi_ideas_package_digit_count(ptr))
    }

    /// Get all digits as a JSON array string.
    public func allDigits() -> String {
        let cstr = divi_ideas_package_all_digits(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Add a digit to the package from JSON.
    public func addDigit(json: String) throws {
        let result = divi_ideas_package_add_digit(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add digit")
        }
    }

    /// Set the package's book (authority) from JSON.
    public func setBook(json: String) throws {
        let result = divi_ideas_package_set_book(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set book")
        }
    }

    /// Set the package's tree (provenance) from JSON.
    public func setTree(json: String) throws {
        let result = divi_ideas_package_set_tree(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set tree")
        }
    }

    /// Set the package's cool (currency value) from JSON.
    public func setCool(json: String) throws {
        let result = divi_ideas_package_set_cool(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set cool")
        }
    }

    /// Set the package's redemption from JSON.
    public func setRedemption(json: String) throws {
        let result = divi_ideas_package_set_redemption(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set redemption")
        }
    }

    /// Set the package's bonds from JSON.
    public func setBonds(json: String) throws {
        let result = divi_ideas_package_set_bonds(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set bonds")
        }
    }

    /// Set the package's position from JSON.
    public func setPosition(json: String) throws {
        let result = divi_ideas_package_set_position(ptr, json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set position")
        }
    }

    /// Read only the header from a .idea directory (no full package load).
    ///
    /// Returns the header as a JSON string.
    public static func readHeader(path: String) throws -> String {
        guard let cstr = divi_ideas_package_read_header(path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to read header from '\(path)'")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - IdeasDigit

/// Stateless digit operations (pure data, JSON round-trip).
///
/// Digits are the content nodes of an .idea file. Each digit has a type,
/// content, author, optional properties, optional children, and timestamps.
/// All operations are copy-on-write -- they return new JSON rather than mutating.
public enum IdeasDigit {

    /// Create a new digit.
    ///
    /// - Parameters:
    ///   - type: The digit type string (e.g. "text", "media.image").
    ///   - contentJSON: A JSON Value for the content.
    ///   - author: The creator's crownId.
    /// - Returns: JSON-encoded Digit.
    public static func new(type: String, contentJSON: String, author: String) throws -> String {
        guard let cstr = divi_ideas_digit_new(type, contentJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create digit")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Return a new digit with updated content (copy-on-write).
    public static func withContent(digitJSON: String, contentJSON: String, by: String) throws -> String {
        guard let cstr = divi_ideas_digit_with_content(digitJSON, contentJSON, by) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update digit content")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Return a new digit with a property set (copy-on-write).
    public static func withProperty(digitJSON: String, key: String, valueJSON: String, by: String) throws -> String {
        guard let cstr = divi_ideas_digit_with_property(digitJSON, key, valueJSON, by) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set digit property '\(key)'")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Return a new digit with a child added (copy-on-write).
    ///
    /// - Parameters:
    ///   - digitJSON: The parent digit as JSON.
    ///   - childId: UUID string of the child digit.
    ///   - by: Author crownId of whoever is making the change.
    public static func withChild(digitJSON: String, childId: String, by: String) throws -> String {
        guard let cstr = divi_ideas_digit_with_child(digitJSON, childId, by) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add child '\(childId)'")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Return a tombstoned (soft-deleted) copy of this digit.
    public static func deleted(digitJSON: String, by: String) throws -> String {
        guard let cstr = divi_ideas_digit_deleted(digitJSON, by) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to delete digit")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Return a restored (un-tombstoned) copy of this digit.
    public static func restored(digitJSON: String, by: String) throws -> String {
        guard let cstr = divi_ideas_digit_restored(digitJSON, by) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to restore digit")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the digit's UUID as a string.
    public static func id(digitJSON: String) throws -> String {
        guard let cstr = divi_ideas_digit_id(digitJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get digit ID")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the digit's type string (e.g. "text", "media.image").
    public static func type(digitJSON: String) throws -> String {
        guard let cstr = divi_ideas_digit_type(digitJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get digit type")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the digit's author string (crownId).
    public static func author(digitJSON: String) throws -> String {
        guard let cstr = divi_ideas_digit_author(digitJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get digit author")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Check if the digit is tombstoned (deleted). Returns false on parse error.
    public static func isDeleted(digitJSON: String) -> Bool {
        divi_ideas_digit_is_deleted(digitJSON)
    }

    /// Check if the digit has children. Returns false on parse error.
    public static func hasChildren(digitJSON: String) -> Bool {
        divi_ideas_digit_has_children(digitJSON)
    }

    /// Extract all text content from a digit for search indexing.
    public static func extractText(digitJSON: String) throws -> String {
        guard let cstr = divi_ideas_digit_extract_text(digitJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to extract text from digit")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Validate a digit (checks type and property keys).
    ///
    /// Throws on validation failure.
    public static func validate(digitJSON: String) throws {
        let result = divi_ideas_digit_validate(digitJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Digit validation failed")
        }
    }
}

// MARK: - IdeasHeader

/// Stateless header operations (pure data, JSON round-trip).
///
/// The header is the metadata envelope of a .idea file: who created it,
/// when, encryption key slots, Babel settings, and sharing recipients.
public enum IdeasHeader {

    /// Create a new header.
    ///
    /// - Parameters:
    ///   - pubkey: Creator's public key string.
    ///   - signature: Creator's signature string.
    ///   - rootId: UUID string of the root digit.
    ///   - keySlotJSON: A JSON-encoded KeySlot for encryption.
    /// - Returns: JSON-encoded Header.
    public static func create(
        pubkey: String,
        signature: String,
        rootId: String,
        keySlotJSON: String
    ) throws -> String {
        guard let cstr = divi_ideas_header_create(pubkey, signature, rootId, keySlotJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create header")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Validate a header. Throws on validation failure.
    public static func validate(headerJSON: String) throws {
        let result = divi_ideas_header_validate(headerJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Header validation failed")
        }
    }

    /// Return a header with updated modified timestamp (copy-on-write).
    ///
    /// Returns the updated header as a JSON string.
    public static func touched(headerJSON: String) throws -> String {
        guard let cstr = divi_ideas_header_touched(headerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to touch header")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Check if Babel obfuscation is enabled. Returns false on parse error.
    public static func isBabelEnabled(headerJSON: String) -> Bool {
        divi_ideas_header_is_babel_enabled(headerJSON)
    }

    /// Check if the header has a password key slot. Returns false on parse error.
    public static func hasPasswordSlot(headerJSON: String) -> Bool {
        divi_ideas_header_has_password_slot(headerJSON)
    }

    /// Get the list of public key recipients who can unlock this idea.
    ///
    /// Returns a JSON array of strings.
    public static func sharedWith(headerJSON: String) throws -> String {
        guard let cstr = divi_ideas_header_shared_with(headerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get shared-with list")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the file extension for this idea (e.g. "idea").
    public static func fileExtension(headerJSON: String) throws -> String {
        guard let cstr = divi_ideas_header_file_extension(headerJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get file extension")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - IdeasValidation

/// Standalone validation helpers for Ideas types.
public enum IdeasValidation {

    /// Validate a digit type string. Throws if invalid.
    public static func validateDigitType(_ typeStr: String) throws {
        let result = divi_ideas_validate_digit_type(typeStr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Invalid digit type: '\(typeStr)'")
        }
    }

    /// Validate a property key string. Throws if invalid.
    public static func validatePropertyKey(_ key: String) throws {
        let result = divi_ideas_validate_property_key(key)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Invalid property key: '\(key)'")
        }
    }

    /// Validate a local bond path. Throws if invalid.
    public static func validateLocalBondPath(_ path: String) throws {
        let result = divi_ideas_validate_local_bond_path(path)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Invalid local bond path: '\(path)'")
        }
    }
}

// MARK: - IdeasSchemaRegistry

/// Swift wrapper around the Rust SchemaRegistry (accumulates and queries digit schemas).
///
/// Schemas define the expected structure of digit types -- required fields,
/// types, defaults, and inheritance chains.
///
/// ```swift
/// let registry = IdeasSchemaRegistry()
/// try registry.register(schemaJSON: mySchemaJSON)
/// let latest = registry.latest(digitType: "media.image")
/// let errors = registry.validateDigit(digitJSON: digit, schemaJSON: schema)
/// ```
public final class IdeasSchemaRegistry: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_ideas_schema_registry_new()
    }

    deinit {
        divi_ideas_schema_registry_free(ptr)
    }

    /// Register a schema from JSON.
    public func register(schemaJSON: String) throws {
        let result = divi_ideas_schema_registry_register(ptr, schemaJSON)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register schema")
        }
    }

    /// Look up a schema by versioned type string (e.g. "media.image@1").
    ///
    /// Returns JSON DigitSchema, or nil if not found.
    public func get(versionedType: String) -> String? {
        guard let cstr = divi_ideas_schema_registry_get(ptr, versionedType) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Look up the latest version of a schema for a given digit type.
    ///
    /// Returns JSON DigitSchema, or nil if not found.
    public func latest(digitType: String) -> String? {
        guard let cstr = divi_ideas_schema_registry_latest(ptr, digitType) else {
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get all registered schemas as a JSON array.
    public func all() -> String {
        let cstr = divi_ideas_schema_registry_all(ptr)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get all versions of a given digit type as a JSON array.
    public func versionsOf(digitType: String) -> String {
        let cstr = divi_ideas_schema_registry_versions_of(ptr, digitType)!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The number of schemas in the registry.
    public var count: Int {
        Int(divi_ideas_schema_registry_len(ptr))
    }

    /// Whether the registry is empty.
    public var isEmpty: Bool {
        divi_ideas_schema_registry_is_empty(ptr)
    }

    /// Resolve a schema by flattening its extends chain using the registry.
    ///
    /// Returns the fully-resolved JSON DigitSchema.
    public func resolve(schemaJSON: String) throws -> String {
        guard let cstr = divi_ideas_schema_registry_resolve(ptr, schemaJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to resolve schema")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Create a new schema at version 1 for the given digit type.
    ///
    /// Returns JSON DigitSchema.
    public static func newSchema(digitType: String) throws -> String {
        guard let cstr = divi_ideas_schema_new(digitType) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create schema for '\(digitType)'")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Validate the schema itself (check defaults match types, property key rules).
    ///
    /// Returns a JSON array of validation error strings. Empty array means valid.
    public static func validateSchema(_ schemaJSON: String) throws -> String {
        guard let cstr = divi_ideas_schema_validate_self(schemaJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to validate schema")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Validate a digit against a schema.
    ///
    /// Returns nil if valid, or a JSON array of validation errors if invalid.
    public func validateDigit(digitJSON: String, schemaJSON: String) throws -> String? {
        let cstr = divi_ideas_schema_validate_digit(digitJSON, schemaJSON)
        if cstr == nil {
            // null means valid -- but check for parse errors first
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr!) }
        return String(cString: cstr!)
    }

    /// Validate a digit against a schema with extends chain resolution.
    ///
    /// Uses this registry to resolve the full inheritance chain.
    /// Returns nil if valid, or a JSON array of validation errors if invalid.
    public func validateComposed(digitJSON: String, schemaJSON: String) throws -> String? {
        let cstr = divi_ideas_schema_validate_composed(digitJSON, schemaJSON, ptr)
        if cstr == nil {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr!) }
        return String(cString: cstr!)
    }
}

// MARK: - IdeasAccessibility

/// Accessibility metadata helpers for digits.
public enum IdeasAccessibility {

    /// Attach accessibility metadata to a digit (copy-on-write).
    ///
    /// - Parameters:
    ///   - digitJSON: The digit to annotate.
    ///   - metaJSON: JSON AccessibilityMetadata (alt text, role, live region, etc.).
    ///   - author: The crownId of whoever is adding the metadata.
    /// - Returns: Updated digit as JSON with a11y_ properties set.
    public static func withAccessibility(digitJSON: String, metaJSON: String, author: String) throws -> String {
        guard let cstr = divi_ideas_digit_with_accessibility(digitJSON, metaJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set accessibility metadata")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Extract accessibility metadata from a digit.
    ///
    /// Returns JSON AccessibilityMetadata, or nil if no a11y metadata is present.
    public static func getAccessibility(digitJSON: String) throws -> String? {
        let cstr = divi_ideas_digit_accessibility(digitJSON)
        if cstr == nil {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr!) }
        return String(cString: cstr!)
    }
}

// MARK: - IdeasBonds

/// Bond operations for .idea inter-file references.
public enum IdeasBonds {

    /// Create empty bonds as JSON.
    public static func new() -> String {
        let cstr = divi_ideas_bonds_new()!
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Count total references across all bond types. Returns 0 on parse error.
    public static func count(json: String) -> Int {
        Int(divi_ideas_bonds_count(json))
    }

    /// Check if bonds are empty. Returns true on parse error.
    public static func isEmpty(json: String) -> Bool {
        divi_ideas_bonds_is_empty(json)
    }

    /// Validate bonds (check local bond paths). Throws on validation error.
    public static func validate(json: String) throws {
        let result = divi_ideas_bonds_validate(json)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Bonds validation failed")
        }
    }
}

// MARK: - IdeasBinding

/// Data binding operations for connecting digits to external data sources.
public enum IdeasBinding {

    /// Attach a data binding to a digit (copy-on-write).
    ///
    /// - Parameters:
    ///   - digitJSON: The digit to bind.
    ///   - bindingJSON: JSON DataBinding describing the data source.
    ///   - author: The crownId of whoever is adding the binding.
    /// - Returns: Updated digit as JSON with binding_ properties set.
    public static func withBinding(digitJSON: String, bindingJSON: String, author: String) throws -> String {
        guard let cstr = divi_ideas_digit_with_binding(digitJSON, bindingJSON, author) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to set data binding")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Extract data binding from a digit.
    ///
    /// Returns JSON DataBinding, or nil if no binding is present.
    public static func parseBinding(digitJSON: String) throws -> String? {
        let cstr = divi_ideas_digit_parse_binding(digitJSON)
        if cstr == nil {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr!) }
        return String(cString: cstr!)
    }
}

// MARK: - IdeasOperation

/// CRDT operation constructors for collaborative editing.
///
/// These create DigitOperation values that can be applied to a shared
/// document state. Each operation carries a vector clock for causal ordering.
public enum IdeasOperation {

    /// Create an Insert CRDT operation.
    ///
    /// - Parameters:
    ///   - digitJSON: JSON value for the digit content being inserted.
    ///   - parentId: UUID string of the parent digit, or nil for root-level.
    ///   - author: The crownId of the author.
    ///   - vectorJSON: JSON VectorClock for causal ordering.
    /// - Returns: JSON DigitOperation.
    public static func insert(
        digitJSON: String,
        parentId: String?,
        author: String,
        vectorJSON: String
    ) throws -> String {
        guard let cstr = divi_ideas_operation_insert(digitJSON, parentId, author, vectorJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create insert operation")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Create an Update CRDT operation.
    ///
    /// - Parameters:
    ///   - digitId: UUID string of the target digit.
    ///   - field: The property name being updated.
    ///   - oldJSON: JSON Value of the old field value.
    ///   - newJSON: JSON Value of the new field value.
    ///   - author: The crownId of the author.
    ///   - vectorJSON: JSON VectorClock for causal ordering.
    /// - Returns: JSON DigitOperation.
    public static func update(
        digitId: String,
        field: String,
        oldJSON: String,
        newJSON: String,
        author: String,
        vectorJSON: String
    ) throws -> String {
        guard let cstr = divi_ideas_operation_update(digitId, field, oldJSON, newJSON, author, vectorJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create update operation")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Create a Delete CRDT operation.
    ///
    /// - Parameters:
    ///   - digitId: UUID string of the target digit.
    ///   - tombstone: Whether this is a soft delete (tombstone) or hard delete.
    ///   - author: The crownId of the author.
    ///   - vectorJSON: JSON VectorClock for causal ordering.
    /// - Returns: JSON DigitOperation.
    public static func delete(
        digitId: String,
        tombstone: Bool,
        author: String,
        vectorJSON: String
    ) throws -> String {
        guard let cstr = divi_ideas_operation_delete(digitId, tombstone, author, vectorJSON) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create delete operation")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - IdeasDomain

/// Domain-specific digit helpers (media, sheet, slide, form, richtext, interactive, commerce).
///
/// Each domain provides `create` functions (meta JSON + author -> digit JSON)
/// and `parse` functions (digit JSON -> meta JSON), plus optional `schema` functions.
///
/// NOTE: These functions are defined in the Rust FFI (ideas_ffi.rs, Wave 3) but are
/// not yet exported in the C header (divinity_ffi.h). The wrappers below are ready
/// to compile once the header is regenerated with cbindgen to include the domain
/// helper functions. Until then, they are compiled out.
///
/// Domain functions follow two patterns:
/// - `divi_ideas_{domain}_{type}_digit(meta_json, author) -> *mut c_char`
/// - `divi_ideas_{domain}_parse_{type}(digit_json) -> *mut c_char`
/// Plus optional schema functions:
/// - `divi_ideas_{domain}_{type}_schema() -> *mut c_char`
///
/// Domains: Media (image, audio, video, stream), Sheet (sheet, cell),
/// Slide (slide), Form (input, checkbox, radio, toggle, dropdown, submit, container),
/// RichText (heading, paragraph, list, blockquote, callout, code, footnote, citation),
/// Interactive (button, navlink, accordion, tabgroup),
/// Commerce (product, storefront, cart_item, order, review).
#if OMNIDEA_DOMAIN_FFI_AVAILABLE

public enum IdeasDomain {

    // MARK: Media

    /// Media digit helpers (image, audio, video, stream).
    public enum Media {

        /// Create an image digit from ImageMeta JSON.
        public static func imageDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_media_image_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create image digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ImageMeta from an image digit.
        public static func parseImage(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_media_parse_image(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse image digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create an audio digit from AudioMeta JSON.
        public static func audioDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_media_audio_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create audio digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse AudioMeta from an audio digit.
        public static func parseAudio(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_media_parse_audio(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse audio digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a video digit from VideoMeta JSON.
        public static func videoDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_media_video_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create video digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse VideoMeta from a video digit.
        public static func parseVideo(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_media_parse_video(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse video digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a stream digit from StreamMeta JSON.
        public static func streamDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_media_stream_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create stream digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse StreamMeta from a stream digit.
        public static func parseStream(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_media_parse_stream(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse stream digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: Sheet

    /// Sheet (spreadsheet) digit helpers.
    public enum Sheet {

        /// Create a sheet digit from SheetMeta JSON.
        public static func sheetDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_sheet_sheet_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create sheet digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse SheetMeta from a sheet digit.
        public static func parseSheet(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_sheet_parse_sheet(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse sheet digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a cell digit from CellMeta JSON.
        public static func cellDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_sheet_cell_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create cell digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CellMeta from a cell digit.
        public static func parseCell(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_sheet_parse_cell(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse cell digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the sheet schema as JSON.
        public static func sheetSchema() -> String {
            let cstr = divi_ideas_sheet_sheet_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the cell schema as JSON.
        public static func cellSchema() -> String {
            let cstr = divi_ideas_sheet_cell_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: Slide

    /// Slide (presentation) digit helpers.
    public enum Slide {

        /// Create a slide digit from SlideMeta JSON.
        public static func slideDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_slide_slide_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create slide digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse SlideMeta from a slide digit.
        public static func parseSlide(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_slide_parse_slide(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse slide digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the slide schema as JSON.
        public static func slideSchema() -> String {
            let cstr = divi_ideas_slide_slide_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: Form

    /// Form digit helpers (input, checkbox, radio, toggle, dropdown, submit, container).
    public enum Form {

        /// Create an input field digit from InputFieldMeta JSON.
        public static func inputDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_input_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create input digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse InputFieldMeta from an input field digit.
        public static func parseInput(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_input(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse input digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a checkbox digit from CheckboxMeta JSON.
        public static func checkboxDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_checkbox_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create checkbox digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CheckboxMeta from a checkbox digit.
        public static func parseCheckbox(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_checkbox(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse checkbox digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a radio button digit from RadioMeta JSON.
        public static func radioDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_radio_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create radio digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse RadioMeta from a radio digit.
        public static func parseRadio(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_radio(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse radio digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a toggle digit from ToggleMeta JSON.
        public static func toggleDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_toggle_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create toggle digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ToggleMeta from a toggle digit.
        public static func parseToggle(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_toggle(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse toggle digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a dropdown digit from DropdownMeta JSON.
        public static func dropdownDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_dropdown_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create dropdown digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse DropdownMeta from a dropdown digit.
        public static func parseDropdown(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_dropdown(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse dropdown digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a submit button digit from SubmitMeta JSON.
        public static func submitDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_submit_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create submit digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse SubmitMeta from a submit digit.
        public static func parseSubmit(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_submit(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse submit digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a form container digit from FormMeta JSON.
        public static func containerDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_form_container_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create form container digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse FormMeta from a form container digit.
        public static func parseContainer(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_form_parse_container(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse form container digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the input field schema as JSON.
        public static func inputSchema() -> String {
            let cstr = divi_ideas_form_input_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the checkbox schema as JSON.
        public static func checkboxSchema() -> String {
            let cstr = divi_ideas_form_checkbox_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the radio schema as JSON.
        public static func radioSchema() -> String {
            let cstr = divi_ideas_form_radio_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the toggle schema as JSON.
        public static func toggleSchema() -> String {
            let cstr = divi_ideas_form_toggle_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the dropdown schema as JSON.
        public static func dropdownSchema() -> String {
            let cstr = divi_ideas_form_dropdown_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the submit schema as JSON.
        public static func submitSchema() -> String {
            let cstr = divi_ideas_form_submit_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the form container schema as JSON.
        public static func containerSchema() -> String {
            let cstr = divi_ideas_form_container_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: RichText

    /// Rich text digit helpers (heading, paragraph, list, blockquote, callout, code, footnote, citation).
    public enum RichText {

        /// Create a heading digit from HeadingMeta JSON.
        public static func headingDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_heading_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create heading digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse HeadingMeta from a heading digit.
        public static func parseHeading(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_heading(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse heading digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a paragraph digit from ParagraphMeta JSON.
        public static func paragraphDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_paragraph_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create paragraph digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ParagraphMeta from a paragraph digit.
        public static func parseParagraph(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_paragraph(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse paragraph digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a list digit from ListMeta JSON.
        public static func listDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_list_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create list digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ListMeta from a list digit.
        public static func parseList(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_list(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse list digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a blockquote digit from BlockquoteMeta JSON.
        public static func blockquoteDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_blockquote_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create blockquote digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse BlockquoteMeta from a blockquote digit.
        public static func parseBlockquote(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_blockquote(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse blockquote digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a callout digit from CalloutMeta JSON.
        public static func calloutDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_callout_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create callout digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CalloutMeta from a callout digit.
        public static func parseCallout(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_callout(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse callout digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a code block digit from CodeBlockMeta JSON.
        public static func codeDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_code_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create code block digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CodeBlockMeta from a code block digit.
        public static func parseCode(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_code(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse code block digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a footnote digit from FootnoteMeta JSON.
        public static func footnoteDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_footnote_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create footnote digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse FootnoteMeta from a footnote digit.
        public static func parseFootnote(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_footnote(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse footnote digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a citation digit from CitationMeta JSON.
        public static func citationDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_richtext_citation_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create citation digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CitationMeta from a citation digit.
        public static func parseCitation(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_richtext_parse_citation(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse citation digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the heading schema as JSON.
        public static func headingSchema() -> String {
            let cstr = divi_ideas_richtext_heading_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the paragraph schema as JSON.
        public static func paragraphSchema() -> String {
            let cstr = divi_ideas_richtext_paragraph_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the list schema as JSON.
        public static func listSchema() -> String {
            let cstr = divi_ideas_richtext_list_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the blockquote schema as JSON.
        public static func blockquoteSchema() -> String {
            let cstr = divi_ideas_richtext_blockquote_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the callout schema as JSON.
        public static func calloutSchema() -> String {
            let cstr = divi_ideas_richtext_callout_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the code block schema as JSON.
        public static func codeSchema() -> String {
            let cstr = divi_ideas_richtext_code_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the footnote schema as JSON.
        public static func footnoteSchema() -> String {
            let cstr = divi_ideas_richtext_footnote_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the citation schema as JSON.
        public static func citationSchema() -> String {
            let cstr = divi_ideas_richtext_citation_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: Interactive

    /// Interactive digit helpers (button, nav link, accordion, tab group).
    public enum Interactive {

        /// Create a button digit from ButtonMeta JSON.
        public static func buttonDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_interactive_button_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create button digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ButtonMeta from a button digit.
        public static func parseButton(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_interactive_parse_button(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse button digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a navigation link digit from NavLinkMeta JSON.
        public static func navLinkDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_interactive_navlink_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create nav link digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse NavLinkMeta from a nav link digit.
        public static func parseNavLink(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_interactive_parse_navlink(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse nav link digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create an accordion digit from AccordionMeta JSON.
        public static func accordionDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_interactive_accordion_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create accordion digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse AccordionMeta from an accordion digit.
        public static func parseAccordion(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_interactive_parse_accordion(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse accordion digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a tab group digit from TabGroupMeta JSON.
        public static func tabGroupDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_interactive_tabgroup_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create tab group digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse TabGroupMeta from a tab group digit.
        public static func parseTabGroup(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_interactive_parse_tabgroup(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse tab group digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the button schema as JSON.
        public static func buttonSchema() -> String {
            let cstr = divi_ideas_interactive_button_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the nav link schema as JSON.
        public static func navLinkSchema() -> String {
            let cstr = divi_ideas_interactive_navlink_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the accordion schema as JSON.
        public static func accordionSchema() -> String {
            let cstr = divi_ideas_interactive_accordion_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the tab group schema as JSON.
        public static func tabGroupSchema() -> String {
            let cstr = divi_ideas_interactive_tabgroup_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }

    // MARK: Commerce

    /// Commerce digit helpers (product, storefront, cart item, order, review).
    public enum Commerce {

        /// Create a product digit from ProductMeta JSON.
        public static func productDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_commerce_product_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create product digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ProductMeta from a product digit.
        public static func parseProduct(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_commerce_parse_product(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse product digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a storefront digit from StorefrontMeta JSON.
        public static func storefrontDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_commerce_storefront_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create storefront digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse StorefrontMeta from a storefront digit.
        public static func parseStorefront(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_commerce_parse_storefront(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse storefront digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a cart item digit from CartItemMeta JSON.
        public static func cartItemDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_commerce_cart_item_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create cart item digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse CartItemMeta from a cart item digit.
        public static func parseCartItem(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_commerce_parse_cart_item(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse cart item digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create an order digit from OrderMeta JSON.
        public static func orderDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_commerce_order_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create order digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse OrderMeta from an order digit.
        public static func parseOrder(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_commerce_parse_order(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse order digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Create a review digit from ReviewMeta JSON.
        public static func reviewDigit(metaJSON: String, author: String) throws -> String {
            guard let cstr = divi_ideas_commerce_review_digit(metaJSON, author) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to create review digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Parse ReviewMeta from a review digit.
        public static func parseReview(digitJSON: String) throws -> String {
            guard let cstr = divi_ideas_commerce_parse_review(digitJSON) else {
                try OmnideaError.check()
                throw OmnideaError(message: "Failed to parse review digit")
            }
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the product schema as JSON.
        public static func productSchema() -> String {
            let cstr = divi_ideas_commerce_product_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the storefront schema as JSON.
        public static func storefrontSchema() -> String {
            let cstr = divi_ideas_commerce_storefront_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the cart item schema as JSON.
        public static func cartItemSchema() -> String {
            let cstr = divi_ideas_commerce_cart_item_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the order schema as JSON.
        public static func orderSchema() -> String {
            let cstr = divi_ideas_commerce_order_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }

        /// Get the review schema as JSON.
        public static func reviewSchema() -> String {
            let cstr = divi_ideas_commerce_review_schema()!
            defer { divi_free_string(cstr) }
            return String(cString: cstr)
        }
    }
}

#endif // OMNIDEA_DOMAIN_FFI_AVAILABLE
