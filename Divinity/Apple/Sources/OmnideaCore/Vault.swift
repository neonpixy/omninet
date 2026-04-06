import COmnideaFFI
import Foundation
import os

private let logger = Logger(subsystem: "co.omnidea", category: "Vault")

// MARK: - Vault

/// Swift wrapper around the Rust Vault (encrypted storage layer).
///
/// Vault manages the key lifecycle, tracks all .idea files in an encrypted
/// manifest database, and supports collectives (multi-user shared spaces).
/// When locked, nothing is accessible. When unlocked, keys exist only in memory.
///
/// ```swift
/// let vault = Vault()
/// try vault.unlock(password: "secret", rootPath: "/path/to/vault")
/// try vault.registerIdea(entry)
/// let ideas = try vault.listIdeas(filter: IdeaFilter())
/// try vault.lock()
/// ```
public final class Vault: @unchecked Sendable {
    private let ptr: OpaquePointer

    public init() {
        ptr = divi_vault_new()!
    }

    deinit {
        divi_vault_free(ptr)
    }

    /// Whether the vault is currently unlocked.
    public var isUnlocked: Bool {
        divi_vault_is_unlocked(ptr)
    }

    // MARK: - Lock / Unlock

    /// Unlock the vault with a password and root directory path.
    ///
    /// This performs the full unlock sequence: derives the master key via PBKDF2,
    /// opens the SQLCipher manifest database, and loads persisted collectives.
    public func unlock(password: String, rootPath: String) throws {
        let result = divi_vault_unlock(ptr, password, rootPath)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to unlock vault")
        }
    }

    /// Lock the vault -- zeros all keys and closes the manifest.
    public func lock() throws {
        let result = divi_vault_lock(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to lock vault")
        }
    }

    // MARK: - Manifest Operations

    /// Register a .idea entry in the manifest.
    public func registerIdea(_ entry: ManifestEntry) throws {
        let data = try JSONEncoder().encode(entry)
        let jsonString = String(data: data, encoding: .utf8)!
        let result = divi_vault_register_idea(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to register idea")
        }
    }

    /// Remove a .idea entry from the manifest by ID.
    public func unregisterIdea(id: String) throws {
        let result = divi_vault_unregister_idea(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to unregister idea '\(id)'")
        }
    }

    /// Get a manifest entry by ID. Returns nil if not found.
    public func getIdea(id: String) throws -> ManifestEntry? {
        guard let json = divi_vault_get_idea(ptr, id) else {
            // Null can mean "not found" or "error" -- check for error.
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(ManifestEntry.self, from: data)
    }

    /// Get a manifest entry by relative path. Returns nil if not found.
    public func getIdeaByPath(_ path: String) throws -> ManifestEntry? {
        guard let json = divi_vault_get_idea_by_path(ptr, path) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(ManifestEntry.self, from: data)
    }

    /// List ideas matching a filter.
    public func listIdeas(filter: IdeaFilter) throws -> [ManifestEntry] {
        let filterData = try JSONEncoder().encode(filter)
        let filterString = String(data: filterData, encoding: .utf8)!

        guard let json = divi_vault_list_ideas(ptr, filterString) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to list ideas")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([ManifestEntry].self, from: data)
    }

    /// List ideas in a folder (path prefix match).
    public func listIdeasInFolder(_ folder: String) throws -> [ManifestEntry] {
        guard let json = divi_vault_list_ideas_in_folder(ptr, folder) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to list ideas in folder '\(folder)'")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([ManifestEntry].self, from: data)
    }

    /// Get the number of registered ideas. Returns -1 on error.
    public func ideaCount() throws -> Int {
        let count = divi_vault_idea_count(ptr)
        if count < 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get idea count")
        }
        return Int(count)
    }

    // MARK: - Encryption

    /// Encrypt data using the content key for a specific idea.
    public func encryptForIdea(_ data: Data, ideaId: String) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = data.withUnsafeBytes { buffer in
            divi_vault_encrypt_for_idea(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                ideaId,
                &outData,
                &outLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to encrypt for idea '\(ideaId)'")
        }

        guard let outData else { return Data() }
        let encrypted = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return encrypted
    }

    /// Decrypt data using the content key for a specific idea.
    public func decryptForIdea(_ data: Data, ideaId: String) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = data.withUnsafeBytes { buffer in
            divi_vault_decrypt_for_idea(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                ideaId,
                &outData,
                &outLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to decrypt for idea '\(ideaId)'")
        }

        guard let outData else { return Data() }
        let decrypted = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return decrypted
    }

    /// Get the content key for a specific idea.
    public func contentKey(ideaId: String) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = divi_vault_content_key(ptr, ideaId, &outKey, &outKeyLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get content key for idea '\(ideaId)'")
        }

        guard let outKey else { return Data() }
        let key = Data(bytes: outKey, count: Int(outKeyLen))
        divi_free_bytes(outKey, outKeyLen)
        return key
    }

    /// Get the vocabulary seed for Babel obfuscation.
    public func vocabularySeed() throws -> Data {
        var outSeed: UnsafeMutablePointer<UInt8>?
        var outSeedLen: UInt = 0

        let result = divi_vault_vocabulary_seed(ptr, &outSeed, &outSeedLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get vocabulary seed")
        }

        guard let outSeed else { return Data() }
        let seed = Data(bytes: outSeed, count: Int(outSeedLen))
        divi_free_bytes(outSeed, outSeedLen)
        return seed
    }

    // MARK: - Collectives

    /// Create a new collective. Generates a random 256-bit key.
    /// Returns the created collective.
    public func createCollective(name: String, ownerPubkey: String) throws -> VaultCollective {
        guard let json = divi_vault_create_collective(ptr, name, ownerPubkey) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create collective '\(name)'")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(VaultCollective.self, from: data)
    }

    /// Join an existing collective with a key received externally.
    ///
    /// - Parameters:
    ///   - id: The collective's UUID string.
    ///   - name: The collective's display name.
    ///   - key: The raw 256-bit collective key.
    ///   - role: Your role in the collective (JSON string, e.g. `"member"`).
    public func joinCollective(id: String, name: String, key: Data, role: String) throws {
        let result = key.withUnsafeBytes { buffer in
            divi_vault_join_collective(
                ptr,
                id,
                name,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count),
                role
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to join collective '\(id)'")
        }
    }

    /// Leave a collective. Removes the key from memory.
    public func leaveCollective(id: String) throws {
        let result = divi_vault_leave_collective(ptr, id)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to leave collective '\(id)'")
        }
    }

    /// List all collectives.
    public func listCollectives() throws -> [VaultCollective] {
        guard let json = divi_vault_list_collectives(ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to list collectives")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([VaultCollective].self, from: data)
    }

    /// Get a collective's encryption key as raw bytes.
    public func collectiveKey(id: String) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = divi_vault_collective_key(ptr, id, &outKey, &outKeyLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to get key for collective '\(id)'")
        }

        guard let outKey else { return Data() }
        let key = Data(bytes: outKey, count: Int(outKeyLen))
        divi_free_bytes(outKey, outKeyLen)
        return key
    }

    // MARK: - Collective Members

    /// Add a member to a collective. Requires Admin or higher.
    ///
    /// - Parameters:
    ///   - collectiveId: The collective's UUID string.
    ///   - pubkey: The new member's public key.
    ///   - role: The member's role (JSON string, e.g. `"member"`, `"admin"`).
    public func collectiveAddMember(collectiveId: String, pubkey: String, role: String) throws {
        let result = divi_vault_collective_add_member(ptr, collectiveId, pubkey, role)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to add member to collective '\(collectiveId)'")
        }
    }

    /// Remove a member from a collective by public key. Requires Owner.
    public func collectiveRemoveMember(collectiveId: String, pubkey: String) throws {
        let result = divi_vault_collective_remove_member(ptr, collectiveId, pubkey)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to remove member from collective '\(collectiveId)'")
        }
    }

    /// Check if a public key is a member of a collective.
    public func collectiveIsMember(collectiveId: String, pubkey: String) -> Bool {
        divi_vault_collective_is_member(ptr, collectiveId, pubkey)
    }

    /// Get a member's role in a collective. Returns nil if not a member.
    public func collectiveMemberRole(collectiveId: String, pubkey: String) throws -> VaultCollectiveRole? {
        guard let json = divi_vault_collective_member_role(ptr, collectiveId, pubkey) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(VaultCollectiveRole.self, from: data)
    }

    // MARK: - Module State

    /// Save a module state entry (encrypted key-value storage for any module).
    public func saveModuleState(moduleId: String, key: String, data: String) throws {
        let result = divi_vault_save_module_state(ptr, moduleId, key, data)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save module state '\(moduleId)/\(key)'")
        }
    }

    /// Load a module state entry. Returns nil if not found.
    public func loadModuleState(moduleId: String, key: String) throws -> String? {
        guard let cstr = divi_vault_load_module_state(ptr, moduleId, key) else {
            try OmnideaError.check()
            return nil
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Delete a module state entry.
    public func deleteModuleState(moduleId: String, key: String) throws {
        let result = divi_vault_delete_module_state(ptr, moduleId, key)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to delete module state '\(moduleId)/\(key)'")
        }
    }

    /// List all state keys for a module.
    public func listModuleStateKeys(moduleId: String) throws -> [String] {
        guard let json = divi_vault_list_module_state_keys(ptr, moduleId) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to list module state keys for '\(moduleId)'")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([String].self, from: data)
    }

    // MARK: - Path Resolution

    /// Get the vault root path. Returns nil if locked.
    public var rootPath: String? {
        guard let cstr = divi_vault_root_path(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the personal ideas directory path. Returns nil if locked.
    public var personalPath: String? {
        guard let cstr = divi_vault_personal_path(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Get the collectives directory path. Returns nil if locked.
    public var collectivesPath: String? {
        guard let cstr = divi_vault_collectives_path(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Resolve a relative path within the vault root. Returns nil if locked.
    public func resolvePath(_ relative: String) -> String? {
        guard let cstr = divi_vault_resolve_path(ptr, relative) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }
}

// MARK: - Codable Types

/// A manifest entry representing a tracked .idea file in the vault.
public struct ManifestEntry: Codable, Sendable {
    public var id: String
    public var path: String
    public var title: String?
    public var extendedType: String?
    public var creator: String
    public var createdAt: String
    public var modifiedAt: String
    public var collectiveId: String?
    public var headerCache: String?

    enum CodingKeys: String, CodingKey {
        case id, path, title, creator
        case extendedType = "extended_type"
        case createdAt = "created_at"
        case modifiedAt = "modified_at"
        case collectiveId = "collective_id"
        case headerCache = "header_cache"
    }

    public init(
        id: String,
        path: String,
        title: String? = nil,
        extendedType: String? = nil,
        creator: String,
        createdAt: String,
        modifiedAt: String,
        collectiveId: String? = nil,
        headerCache: String? = nil
    ) {
        self.id = id
        self.path = path
        self.title = title
        self.extendedType = extendedType
        self.creator = creator
        self.createdAt = createdAt
        self.modifiedAt = modifiedAt
        self.collectiveId = collectiveId
        self.headerCache = headerCache
    }
}

/// Filter criteria for querying manifest entries.
///
/// All fields are optional. An entry matches if it satisfies ALL
/// specified criteria (AND logic). An empty filter matches everything.
public struct IdeaFilter: Codable, Sendable {
    public var creator: String?
    public var collectiveId: String?
    public var extendedType: String?
    public var modifiedAfter: String?
    public var modifiedBefore: String?
    public var pathPrefix: String?
    public var titleContains: String?

    enum CodingKeys: String, CodingKey {
        case creator
        case collectiveId = "collective_id"
        case extendedType = "extended_type"
        case modifiedAfter = "modified_after"
        case modifiedBefore = "modified_before"
        case pathPrefix = "path_prefix"
        case titleContains = "title_contains"
    }

    public init(
        creator: String? = nil,
        collectiveId: String? = nil,
        extendedType: String? = nil,
        modifiedAfter: String? = nil,
        modifiedBefore: String? = nil,
        pathPrefix: String? = nil,
        titleContains: String? = nil
    ) {
        self.creator = creator
        self.collectiveId = collectiveId
        self.extendedType = extendedType
        self.modifiedAfter = modifiedAfter
        self.modifiedBefore = modifiedBefore
        self.pathPrefix = pathPrefix
        self.titleContains = titleContains
    }
}

/// A shared space among multiple members.
public struct VaultCollective: Codable, Sendable, Identifiable {
    public var id: String
    public var name: String
    public var createdAt: String
    public var members: [VaultCollectiveMember]
    public var ourRole: VaultCollectiveRole

    enum CodingKeys: String, CodingKey {
        case id, name, members
        case createdAt = "created_at"
        case ourRole = "our_role"
    }
}

/// A member of a collective.
public struct VaultCollectiveMember: Codable, Sendable {
    public var publicKey: String
    public var joinedAt: String
    public var role: VaultCollectiveRole

    enum CodingKeys: String, CodingKey {
        case role
        case publicKey = "public_key"
        case joinedAt = "joined_at"
    }
}

/// Role within a collective. Matches the Rust enum's lowercase serde format.
public enum VaultCollectiveRole: String, Codable, Sendable, Comparable {
    case readonly
    case member
    case admin
    case owner

    private var level: Int {
        switch self {
        case .readonly: 1
        case .member: 2
        case .admin: 3
        case .owner: 4
        }
    }

    public static func < (lhs: VaultCollectiveRole, rhs: VaultCollectiveRole) -> Bool {
        lhs.level < rhs.level
    }
}
