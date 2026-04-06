import COmnideaFFI
import Foundation

// MARK: - Encryption

/// Sentinal encryption — AES-256-GCM via the Rust core.
public enum SentinalCrypto {

    /// Encrypt plaintext with a 32-byte key.
    /// Returns combined format (nonce || ciphertext || tag).
    public static func encrypt(_ plaintext: Data, key: Data) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = plaintext.withUnsafeBytes { ptBuf in
            key.withUnsafeBytes { keyBuf in
                divi_sentinal_encrypt(
                    ptBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(ptBuf.count),
                    keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(keyBuf.count),
                    &outData, &outLen
                )
            }
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Encryption failed")
        }

        guard let outData else { return Data() }
        let encrypted = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return encrypted
    }

    /// Decrypt combined-format ciphertext with a 32-byte key.
    public static func decrypt(_ ciphertext: Data, key: Data) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = ciphertext.withUnsafeBytes { ctBuf in
            key.withUnsafeBytes { keyBuf in
                divi_sentinal_decrypt(
                    ctBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(ctBuf.count),
                    keyBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(keyBuf.count),
                    &outData, &outLen
                )
            }
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Decryption failed")
        }

        guard let outData else { return Data() }
        let decrypted = Data(bytes: outData, count: Int(outLen))
        divi_free_bytes(outData, outLen)
        return decrypted
    }
}

// MARK: - Key Derivation

public enum SentinalKeys {

    /// Derive a master key from a password. Returns (key, salt).
    /// If salt is nil, generates a random 32-byte salt.
    public static func deriveMasterKey(password: String, salt: Data? = nil) throws -> (key: Data, salt: Data) {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0
        var outSalt: UnsafeMutablePointer<UInt8>?
        var outSaltLen: UInt = 0

        let result: Int32
        if let salt {
            result = salt.withUnsafeBytes { sBuf in
                divi_sentinal_derive_master_key(
                    password, sBuf.baseAddress?.assumingMemoryBound(to: UInt8.self), UInt(sBuf.count),
                    &outKey, &outKeyLen, &outSalt, &outSaltLen
                )
            }
        } else {
            result = divi_sentinal_derive_master_key(
                password, nil, 0,
                &outKey, &outKeyLen, &outSalt, &outSaltLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Master key derivation failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        let saltOut = Data(bytes: outSalt!, count: Int(outSaltLen))
        divi_free_bytes(outKey!, outKeyLen)
        divi_free_bytes(outSalt!, outSaltLen)
        return (key, saltOut)
    }

    /// Derive a content key from a master key and a UUID.
    public static func deriveContentKey(masterKey: Data, id: String) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = masterKey.withUnsafeBytes { mkBuf in
            divi_sentinal_derive_content_key(
                mkBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(mkBuf.count), id, &outKey, &outKeyLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Content key derivation failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        divi_free_bytes(outKey!, outKeyLen)
        return key
    }

    /// Derive a shared key from an ECDH shared secret.
    public static func deriveSharedKey(sharedSecret: Data) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = sharedSecret.withUnsafeBytes { sBuf in
            divi_sentinal_derive_shared_key(
                sBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(sBuf.count), &outKey, &outKeyLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Shared key derivation failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        divi_free_bytes(outKey!, outKeyLen)
        return key
    }

    /// Generate a random salt of the given length.
    public static func generateSalt(length: Int = 32) throws -> Data {
        var outData: UnsafeMutablePointer<UInt8>?
        var outLen: UInt = 0

        let result = divi_sentinal_generate_salt(UInt(length), &outData, &outLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Salt generation failed")
        }

        let salt = Data(bytes: outData!, count: Int(outLen))
        divi_free_bytes(outData!, outLen)
        return salt
    }
}

// MARK: - Identity Key Derivation

public enum SentinalIdentity {

    /// Derive a 32-byte identity key from a BIP-39 seed using HKDF-SHA256.
    ///
    /// Takes the 64-byte seed from `SentinalRecovery.phraseToSeed()` and produces
    /// a 32-byte key suitable for use as a secp256k1 private key.
    /// Uses domain salt `"omnidea-identity-v1"` with info `"identity-primary"`.
    public static func deriveIdentityKey(seed: Data) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = seed.withUnsafeBytes { seedBuf in
            divi_sentinal_derive_identity_key(
                seedBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(seedBuf.count),
                &outKey, &outKeyLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Identity key derivation failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        divi_free_bytes(outKey!, outKeyLen)
        return key
    }
}

// MARK: - Key Slots

public enum SentinalKeySlots {

    /// Create a password-protected key slot wrapping a content key.
    /// Returns the KeySlot as a JSON string (for storage/transmission).
    public static func createPasswordSlot(contentKey: Data, password: String) throws -> String {
        let json: UnsafeMutablePointer<CChar>? = contentKey.withUnsafeBytes { ckBuf in
            divi_sentinal_key_slot_create_password(
                ckBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(ckBuf.count), password
            )
        }

        guard let json else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create password key slot")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Create a public-key key slot (X25519 ECDH) wrapping a content key.
    /// `recipientPubkey` must be 32 bytes (X25519 public key).
    public static func createPublicKeySlot(
        contentKey: Data, recipientPubkey: Data, recipientCrownId: String
    ) throws -> String {
        let json: UnsafeMutablePointer<CChar>? = contentKey.withUnsafeBytes { ckBuf in
            recipientPubkey.withUnsafeBytes { rpBuf in
                divi_sentinal_key_slot_create_public(
                    ckBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    UInt(ckBuf.count),
                    rpBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    recipientCrownId
                )
            }
        }

        guard let json else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create public key slot")
        }
        defer { divi_free_string(json) }
        return String(cString: json)
    }

    /// Unwrap a key slot with a password. Returns the content key bytes.
    public static func unwrapWithPassword(slotJSON: String, password: String) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = divi_sentinal_key_slot_unwrap_password(slotJSON, password, &outKey, &outKeyLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Key slot unwrap failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        divi_free_bytes(outKey!, outKeyLen)
        return key
    }

    /// Unwrap a key slot with a private key (32 bytes X25519).
    public static func unwrapWithPrivateKey(slotJSON: String, privateKey: Data) throws -> Data {
        var outKey: UnsafeMutablePointer<UInt8>?
        var outKeyLen: UInt = 0

        let result = privateKey.withUnsafeBytes { pkBuf in
            divi_sentinal_key_slot_unwrap_private(
                slotJSON,
                pkBuf.baseAddress?.assumingMemoryBound(to: UInt8.self),
                &outKey, &outKeyLen
            )
        }

        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Key slot unwrap failed")
        }

        let key = Data(bytes: outKey!, count: Int(outKeyLen))
        divi_free_bytes(outKey!, outKeyLen)
        return key
    }
}

// MARK: - Recovery

public enum SentinalRecovery {

    /// Generate a random 24-word recovery phrase.
    public static func generatePhrase() throws -> [String] {
        guard let json = divi_sentinal_recovery_generate() else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to generate recovery phrase")
        }
        defer { divi_free_string(json) }
        let data = Data(String(cString: json).utf8)
        return try JSONDecoder().decode([String].self, from: data)
    }

    /// Validate a recovery phrase.
    public static func validatePhrase(_ words: [String]) -> Bool {
        guard let jsonData = try? JSONEncoder().encode(words),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            return false
        }
        return divi_sentinal_recovery_validate(jsonString)
    }

    /// Convert a recovery phrase to a 64-byte seed.
    public static func phraseToSeed(_ words: [String], passphrase: String = "") throws -> Data {
        guard let jsonData = try? JSONEncoder().encode(words),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            throw OmnideaError(message: "Failed to encode phrase")
        }

        var outSeed: UnsafeMutablePointer<UInt8>?
        var outSeedLen: UInt = 0

        let result = divi_sentinal_recovery_to_seed(jsonString, passphrase, &outSeed, &outSeedLen)
        if result != 0 {
            try OmnideaError.check()
            throw OmnideaError(message: "Phrase to seed conversion failed")
        }

        let seed = Data(bytes: outSeed!, count: Int(outSeedLen))
        divi_free_bytes(outSeed!, outSeedLen)
        return seed
    }
}
