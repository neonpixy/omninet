# Sentinal -- Encryption

The guardian. Every secret in Omninet is protected by Sentinal. Keys are derived, never stored raw. Passwords never travel over Equipment. Memory is zeroed when no longer needed.

## Key Hierarchy

```
Password (user enters)
    | PBKDF2 (600K iterations, HMAC-SHA256)
    v
Master Key (256-bit)
    |-- HKDF(salt: "omnidea-content-v1")    -> Content Key (per .idea)
    |-- HKDF(salt: "omnidea-vocabulary-v1") -> Vocabulary Seed (for Lingo's Babel)
    |-- HKDF(salt: "omnidea-dimension-v1")  -> Dimension Key (per vault section)
    |-- HKDF(salt: "omnidea-share-v1")      -> Shared Key (X25519 ECDH for PublicKeySlot)
    |-- HKDF(salt: "omnidea-storage-v1")    -> Storage Key (SQLCipher for Tower/Omnibus)
    +-- HKDF(salt: "omnidea-soul-v1")       -> Soul Key (AES-256-GCM for Crown soul.json)
```

## Crypto Parameters

- **PBKDF2**: HMAC-SHA256, 600,000 iterations, 32-byte output, 32-byte random salt
- **HKDF**: SHA-256, 7 domain-separated salt strings (`omnidea-*-v1`)
- **AES-256-GCM**: 12-byte nonce (fresh random), 16-byte tag, combined format: `nonce || ciphertext || tag`
- **X25519**: Ephemeral ECDH for PublicKeySlot, shared secret -> HKDF -> wrapping key
- **BIP-39**: 24-word mnemonic from 32 bytes entropy, PBKDF2-SHA512 seed derivation

## Modules

### `key_derivation` -- Key Derivation

- `derive_master_key(password, salt?)` -> (SecureData, salt). PBKDF2-HMAC-SHA256, 600K iterations.
- `derive_content_key(master, idea_id)` -> per-idea content key via HKDF.
- `derive_vocabulary_seed(master)` -> seed for Lingo's Babel text obfuscation.
- `derive_dimension_key(master, dimension_id)` -> per-vault-section key via HKDF.
- `derive_shared_key(shared_secret)` -> wrapping key from X25519 ECDH shared secret.
- `derive_storage_key(private_key_bytes, context)` -> SQLCipher key for Tower/Omnibus relay databases. Context parameter provides domain separation (e.g., "tower-relay", "omnibus-relay").
- `derive_soul_key(master_key)` -> soul encryption key for Crown's soul.json at-rest encryption. Salt: `"omnidea-soul-v1"`, Info: `"soul-data"`.
- `generate_salt(length)` -> cryptographically random bytes.
- Constants: `KEY_LENGTH = 32`, `SALT_LENGTH = 32`, `PBKDF2_ITERATIONS = 600_000`.

### `encryption` -- AES-256-GCM

- `encrypt(plaintext, key)` -> EncryptedData (structured: ciphertext + nonce + tag).
- `decrypt(encrypted, key)` -> plaintext.
- `encrypt_combined(plaintext, key)` -> combined bytes (nonce || ciphertext || tag).
- `decrypt_combined(combined, key)` -> plaintext.
- **EncryptedData** -- Serialize/Deserialize struct. `combined()` -> bytes. `from_combined(bytes)` -> parse.

### `key_slot` -- Three Unlock Paths

- **KeySlot** -- Tagged enum (serde `#[serde(tag = "type")]`): Password, PublicKey, Internal.
- **KeySlotCredential** -- Enum: Password(&str), PrivateKey(&[u8; 32]), DimensionKey(&[u8]).
- **PasswordKeySlot** -- PBKDF2 derives wrapping key from password -> AES-GCM wraps content key. Stores salt + wrapped_key.
- **PublicKeySlot** -- Ephemeral X25519 ECDH -> HKDF -> wrapping key -> AES-GCM wraps content key. Stores recipient crown_id + ephemeral public key + wrapped_key.
- **InternalKeySlot** -- Direct AES-GCM wrap with dimension key. Stores dimension_id (Uuid) + wrapped_key.
- `KeySlot::unwrap(credential)` -> dispatches to the matching slot type, returns `SecureData`. Returns `CredentialMismatch` on type mismatch.

### `secure_data` -- Memory-Safe Containers

- **SecureData** -- Zeroize-on-drop (via `zeroize` crate), constant-time equality (via `subtle` crate), redacted Debug/Display. Access requires explicit `expose()` call. Constructors: `new(Vec<u8>)`, `from_slice(&[u8])`, `random(length)`.

### `recovery` -- BIP-39 Mnemonic Phrases

- `generate_phrase()` -> 24 words from 32 bytes entropy.
- `validate_phrase(words)` -> checksum verification.
- `phrase_to_seed(words, passphrase)` -> 64-byte seed (PBKDF2-HMAC-SHA512 per BIP-39 spec).
- `wordlist()` -> 2048 BIP-39 English words.

### `obfuscation` -- Defense-in-Depth Layer

NOT encryption -- a defense-in-depth layer for protecting data in memory after decryption.

- `obfuscate(data, seed)` / `deobfuscate(data, seed)` -- Binary XOR with HMAC-SHA256 counter-mode keystream. Self-reversing.
- `scramble_colors(pixels, seed)` / `unscramble_colors(pixels, seed)` -- Seed-deterministic Fisher-Yates permutation of byte values 0-255.
- `generate_shuffle_pattern(count, seed, idea_id)` / `generate_reverse_shuffle_pattern(...)` -- Per-idea pixel position permutation via Fisher-Yates.

### `random` -- Deterministic PRNG

- **SeededRandom** -- xorshift64. SHA-256 seed initialization. NOT cryptographically secure -- used only for deterministic Fisher-Yates shuffles in obfuscation. Methods: `next()` -> non-negative usize, `next_bounded(bound)` -> 0..bound.

## Public Re-exports

```rust
pub use encryption::EncryptedData;
pub use error::SentinalError;
pub use key_slot::{InternalKeySlot, KeySlot, KeySlotCredential, PasswordKeySlot, PublicKeySlot};
pub use random::SeededRandom;
pub use secure_data::SecureData;
```

## SentinalError

9 variants: InvalidCombinedData, InvalidWrappedKey, CredentialMismatch, DecryptionFailed, KeyDerivationFailed, InvalidKeyLength, InvalidSalt, InvalidRecoveryPhrase, RandomGenerationFailed, Serialization (From<serde_json::Error>). Send + Sync.

## Dependencies

External only (zero internal Omninet deps):

```toml
aes-gcm, pbkdf2, hkdf, sha2, hmac          # RustCrypto
x25519-dalek                                 # ECDH
zeroize, subtle                              # Memory safety
bip39                                        # Recovery phrases
getrandom                                    # OS randomness
serde, serde_json, thiserror, uuid, log      # Standard workspace deps
```

## What Does NOT Live Here

- **Vocabulary/word mapping** -> Lingo (linguistic, not cryptographic)
- **TranslationKit/vocab sharing** -> Lingo + Globe
- **NIP-04/NIP-44** -> Globe (Globe-specific encryption)
- **SecureKeyStore/BiometricGate** -> Divinity/Apple (platform-specific)
- **KeyVault (in-memory registry)** -> Vault (key management, not primitives)

## Covenant Alignment

**Sovereignty** -- your keys, your data. No backdoors, no escrow. **Dignity** -- obfuscation ensures even in-memory content is protected from side-channel exposure. **Consent** -- key slots mean you choose who gets access.
