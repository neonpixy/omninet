import COmnideaFFI
import Foundation

// MARK: - Keyring

/// Swift wrapper around the Rust Keyring (cryptographic identity management).
public final class Keyring: @unchecked Sendable {
    let ptr: OpaquePointer

    public init() {
        ptr = divi_crown_keyring_new()!
    }

    /// Internal init from a pre-existing opaque pointer (used by recovery).
    init(recovered pointer: OpaquePointer) {
        self.ptr = pointer
    }

    deinit {
        divi_crown_keyring_free(ptr)
    }

    /// Whether a primary identity is loaded (unlocked).
    public var isUnlocked: Bool {
        divi_crown_keyring_is_unlocked(ptr)
    }

    /// Generate a new random primary keypair.
    /// Returns the new identity info (crownId, cpub_hex).
    public func generatePrimary() throws -> KeypairInfo {
        guard let json = divi_crown_keyring_generate_primary(ptr) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to generate primary keypair")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(KeypairInfo.self, from: data)
    }

    /// Import a primary keypair from an crownSecret bech32 string.
    public func importPrimary(crownSecret: String) throws {
        let result = divi_crown_keyring_import_primary(ptr, crownSecret)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to import primary keypair")
        }
    }

    /// The primary identity's crownId string, or nil if locked.
    public var publicKey: String? {
        guard let cstr = divi_crown_keyring_public_key(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// The primary identity's public key as a 64-char hex string, or nil if locked.
    public var publicKeyHex: String? {
        guard let cstr = divi_crown_keyring_public_key_hex(ptr) else { return nil }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Sign data with the primary identity.
    public func sign(_ data: Data) throws -> CrownSignature {
        let json: UnsafeMutablePointer<CChar>? = data.withUnsafeBytes { buffer in
            divi_crown_keyring_sign(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }

        guard let json else {
            try OmnideaError.check()
            throw OmnideaError(message: "Signing failed")
        }
        defer { divi_free_string(json) }
        let jsonData = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(CrownSignature.self, from: jsonData)
    }

    /// Export the keyring as bytes (hex-encoded private keys in JSON).
    /// The caller should encrypt these bytes before persisting.
    public func export() throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = divi_crown_keyring_export(ptr, &outData, &outLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Keyring export failed")
        }

        guard let outData else { return Data() }
        let exported = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return exported
    }

    /// Load keyring state from previously exported bytes.
    public func load(_ data: Data) throws {
        let result = data.withUnsafeBytes { buffer in
            divi_crown_keyring_load(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Keyring load failed")
        }
    }

    /// Lock the keyring — clear all keys from memory.
    public func lock() {
        divi_crown_keyring_lock(ptr)
    }

    /// List all persona names, sorted alphabetically.
    public func listPersonas() -> [String] {
        guard let json = divi_crown_keyring_list_personas(ptr) else { return [] }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return (try? JSONDecoder().decode([String].self, from: data)) ?? []
    }

    /// Create a new persona keypair. Returns the persona's crownId.
    public func createPersona(_ name: String) throws -> String {
        guard let cstr = divi_crown_keyring_create_persona(ptr, name) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create persona '\(name)'")
        }
        defer { divi_free_string(cstr) }
        return String(cString: cstr)
    }

    /// Delete a named persona.
    public func deletePersona(_ name: String) throws {
        let result = divi_crown_keyring_delete_persona(ptr, name)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to delete persona '\(name)'")
        }
    }

    /// Compute an ECDH shared secret with another party's public key.
    ///
    /// Both sides independently arrive at the same 32-byte shared secret.
    /// Used for encrypted DMs — derive an encryption key from the shared secret.
    public func sharedSecret(theirPublicKey: Data) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = theirPublicKey.withUnsafeBytes { buffer in
            divi_crown_shared_secret(
                ptr,
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                &outData,
                &outLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "ECDH shared secret failed")
        }

        guard let outData else { return Data() }
        let secret = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return secret
    }
}

// MARK: - Recovery

extension Keyring {
    /// Recover a Keyring from a raw 32-byte private key.
    ///
    /// Takes identity key bytes (e.g., from `SentinalIdentity.deriveIdentityKey`)
    /// and produces a new Keyring with that key imported as the primary identity.
    ///
    /// Full recovery path:
    /// ```swift
    /// let seed = try SentinalRecovery.phraseToSeed(words)
    /// let identityKey = try SentinalIdentity.deriveIdentityKey(seed: seed)
    /// let keyring = try Keyring.recoverFromSecret(identityKey)
    /// ```
    public static func recoverFromSecret(_ secret: Data) throws -> Keyring {
        guard secret.count == 32 else {
            throw OmnideaError(message: "Recovery secret must be exactly 32 bytes")
        }

        let result: OpaquePointer? = secret.withUnsafeBytes { buffer in
            divi_crown_recover_from_secret(
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }

        guard let result else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to recover keyring from secret")
        }

        return Keyring(recovered: result)
    }
}

// MARK: - Soul

/// Swift wrapper around the Rust Soul (identity container: profile + preferences + social graph).
public final class Soul: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Create a new in-memory soul with defaults.
    public init() {
        ptr = divi_crown_soul_new()!
    }

    /// Internal init from an opaque pointer.
    private init(pointer: OpaquePointer) {
        self.ptr = pointer
    }

    deinit {
        divi_crown_soul_free(ptr)
    }

    /// Create a new soul at the given path, writing soul.json with defaults.
    public static func create(at path: String) throws -> Soul {
        guard let soulPtr: OpaquePointer = divi_crown_soul_create(path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create soul at '\(path)'")
        }
        return Soul(pointer: soulPtr)
    }

    /// Load an existing soul from a directory (reads soul.json).
    public static func load(from path: String) throws -> Soul {
        guard let soulPtr: OpaquePointer = divi_crown_soul_load(path) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to load soul from '\(path)'")
        }
        return Soul(pointer: soulPtr)
    }

    /// Save the soul to disk.
    public func save() throws {
        let result = divi_crown_soul_save(ptr)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to save soul")
        }
    }

    /// Whether unsaved changes exist.
    public var isDirty: Bool {
        divi_crown_soul_is_dirty(ptr)
    }

    // MARK: Profile

    /// Get the profile as a decoded struct.
    public func profile() throws -> CrownProfile {
        guard let json = divi_crown_soul_profile(ptr) else {
            throw OmnideaError(message: "Failed to get profile")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(CrownProfile.self, from: data)
    }

    /// Update the profile.
    public func updateProfile(_ profile: CrownProfile) throws {
        let data = try JSONEncoder().encode(profile)
        let jsonString = String(data: data, encoding: .utf8)!
        let result = divi_crown_soul_update_profile(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update profile")
        }
    }

    // MARK: Preferences

    /// Get preferences as a decoded struct.
    public func preferences() throws -> CrownPreferences {
        guard let json = divi_crown_soul_preferences(ptr) else {
            throw OmnideaError(message: "Failed to get preferences")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(CrownPreferences.self, from: data)
    }

    /// Update preferences.
    public func updatePreferences(_ prefs: CrownPreferences) throws {
        let data = try JSONEncoder().encode(prefs)
        let jsonString = String(data: data, encoding: .utf8)!
        let result = divi_crown_soul_update_preferences(ptr, jsonString)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to update preferences")
        }
    }

    // MARK: Social Graph

    /// Get the full social graph as a decoded struct.
    public func socialGraph() throws -> CrownSocialGraph {
        guard let json = divi_crown_soul_social_graph(ptr) else {
            throw OmnideaError(message: "Failed to get social graph")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode(CrownSocialGraph.self, from: data)
    }

    /// Follow an crownId.
    public func follow(_ crownId: String) {
        divi_crown_soul_follow(ptr, crownId)
    }

    /// Unfollow an crownId.
    public func unfollow(_ crownId: String) {
        divi_crown_soul_unfollow(ptr, crownId)
    }

    /// Block an crownId (also removes from following).
    public func block(_ crownId: String) {
        divi_crown_soul_block(ptr, crownId)
    }

    /// Unblock an crownId.
    public func unblock(_ crownId: String) {
        divi_crown_soul_unblock(ptr, crownId)
    }

    /// Mute an crownId.
    public func mute(_ crownId: String) {
        divi_crown_soul_mute(ptr, crownId)
    }

    /// Unmute an crownId.
    public func unmute(_ crownId: String) {
        divi_crown_soul_unmute(ptr, crownId)
    }

    /// Check if an crownId is followed.
    public func isFollowing(_ crownId: String) -> Bool {
        divi_crown_soul_is_following(ptr, crownId)
    }

    /// Check if an crownId is blocked.
    public func isBlocked(_ crownId: String) -> Bool {
        divi_crown_soul_is_blocked(ptr, crownId)
    }

    /// Check if an crownId is muted.
    public func isMuted(_ crownId: String) -> Bool {
        divi_crown_soul_is_muted(ptr, crownId)
    }
}

// MARK: - Codable Types

/// Info returned when generating a keypair.
public struct KeypairInfo: Codable, Sendable {
    public let crownId: String
    public let cpubHex: String

    enum CodingKeys: String, CodingKey {
        case crownId
        case cpubHex = "cpub_hex"
    }
}

/// A BIP-340 Schnorr signature with metadata.
public struct CrownSignature: Codable, Sendable {
    public let data: String
    public let signer: String
    public let timestamp: String
}

/// User-facing profile metadata.
public struct CrownProfile: Codable, Sendable {
    public var displayName: String?
    public var username: String?
    public var bio: String?
    public var avatar: CrownAvatarReference?
    public var banner: CrownAvatarReference?
    public var website: String?
    public var language: String
    public var lightningAddress: String?
    public var nip05: String?
    public var updatedAt: String

    enum CodingKeys: String, CodingKey {
        case displayName = "display_name"
        case username, bio, avatar, banner, website, language
        case lightningAddress = "lightning_address"
        case nip05
        case updatedAt = "updated_at"
    }
}

/// Avatar reference — tagged enum matching Rust's AvatarReference.
public enum CrownAvatarReference: Codable, Sendable {
    case data(bytes: String, mimeType: String)
    case asset(ideaId: String, assetName: String)
    case url(String)

    enum CodingKeys: String, CodingKey {
        case type
        case data, mimeType = "mime_type"
        case ideaId = "idea_id", assetName = "asset_name"
        case url
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "data":
            let bytes = try container.decode(String.self, forKey: .data)
            let mime = try container.decode(String.self, forKey: .mimeType)
            self = .data(bytes: bytes, mimeType: mime)
        case "asset":
            let id = try container.decode(String.self, forKey: .ideaId)
            let name = try container.decode(String.self, forKey: .assetName)
            self = .asset(ideaId: id, assetName: name)
        case "url":
            let urlStr = try container.decode(String.self, forKey: .url)
            self = .url(urlStr)
        default:
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "Unknown avatar type: \(type)"
            ))
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .data(let bytes, let mime):
            try container.encode("data", forKey: .type)
            try container.encode(bytes, forKey: .data)
            try container.encode(mime, forKey: .mimeType)
        case .asset(let id, let name):
            try container.encode("asset", forKey: .type)
            try container.encode(id, forKey: .ideaId)
            try container.encode(name, forKey: .assetName)
        case .url(let urlStr):
            try container.encode("url", forKey: .type)
            try container.encode(urlStr, forKey: .url)
        }
    }
}

/// User preferences.
public struct CrownPreferences: Codable, Sendable {
    public var theme: String
    public var textScale: Double
    public var reduceMotion: Bool
    public var contentLanguage: String
    public var interfaceLanguage: String?
    public var autoTranslate: Bool
    public var defaultVisibility: String
    public var showOnlineStatus: Bool
    public var sendReadReceipts: Bool
    public var pushEnabled: Bool
    public var notificationCategories: [String]

    enum CodingKeys: String, CodingKey {
        case theme
        case textScale = "text_scale"
        case reduceMotion = "reduce_motion"
        case contentLanguage = "content_language"
        case interfaceLanguage = "interface_language"
        case autoTranslate = "auto_translate"
        case defaultVisibility = "default_visibility"
        case showOnlineStatus = "show_online_status"
        case sendReadReceipts = "send_read_receipts"
        case pushEnabled = "push_enabled"
        case notificationCategories = "notification_categories"
    }
}

/// Social connections graph.
public struct CrownSocialGraph: Codable, Sendable {
    public var following: [String]
    public var followers: [String]
    public var blocked: [String]
    public var muted: [String]
    public var trusted: [String]
    public var lists: [String: [String]]
}
