import COmnideaFFI
import Foundation

/// Babel — hardened semantic text obfuscation.
///
/// Transforms text into sequences of Unicode symbols from ancient and
/// exotic scripts using a seed-derived vocabulary. Non-deterministic:
/// same input produces different output each time.
///
/// Create a shared Babel instance between two parties using their
/// ECDH shared secret as the seed.
///
/// ```swift
/// let sharedSecret = try keyring.sharedSecret(theirPublicKey: theirPubkey)
/// let babel = Babel(seed: sharedSecret)
/// let encoded = babel.encode("Hello!")   // → ancient Unicode symbols
/// let decoded = babel.decode(encoded)    // → "Hello!"
/// ```
public final class Babel: @unchecked Sendable {
    private let ptr: OpaquePointer

    /// Create a Babel instance from a vocabulary seed.
    ///
    /// The seed is typically a 32-byte ECDH shared secret or
    /// Sentinal-derived vocabulary key.
    public init(seed: Data) throws {
        guard let p: OpaquePointer = seed.withUnsafeBytes({ buffer in
            divi_lingo_babel_new(
                buffer.baseAddress?.assumingMemoryBound(to: UInt8.self),
                UInt(buffer.count)
            )
        }) else {
            try OmnideaError.check()
            throw OmnideaError(message: "Failed to create Babel")
        }
        ptr = p
    }

    deinit {
        divi_lingo_babel_free(ptr)
    }

    /// Encode text into Babel symbols (hardened, non-deterministic).
    ///
    /// Same input produces different output each time due to
    /// random nonce and homophone selection.
    public func encode(_ text: String) -> String {
        guard let encoded = divi_lingo_babel_encode(ptr, text) else { return "" }
        defer { divi_free_string(encoded) }
        return String(cString: encoded)
    }

    /// Decode Babel symbols back into plaintext.
    public func decode(_ encoded: String) -> String {
        guard let decoded = divi_lingo_babel_decode(ptr, encoded) else { return "" }
        defer { divi_free_string(decoded) }
        return String(cString: decoded)
    }
}
