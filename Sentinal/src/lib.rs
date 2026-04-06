//! Sentinal — Encryption primitives for Omnidea.
//!
//! The guardian. Every secret in Omnidea is protected by Sentinal.
//! Keys are derived, never stored raw. Passwords never travel over
//! Equipment. Memory is zeroed when no longer needed.
//!
//! # What Sentinal provides
//!
//! - **Key derivation**: PBKDF2 (600K iterations) + HKDF-SHA256
//! - **Encryption**: AES-256-GCM with combined format
//! - **Key slots**: Password, public key (X25519), and dimension key unlock paths
//! - **Secure memory**: Zeroize-on-drop containers with constant-time equality
//! - **Recovery**: BIP-39 24-word mnemonic phrases
//! - **Obfuscation**: Binary XOR keystream + image color/pixel scrambling
//! - **Deterministic PRNG**: xorshift64 for reproducible shuffles

/// AES-256-GCM encryption and decryption (structured and combined formats).
pub mod encryption;
/// Error types for all Sentinal operations.
pub mod error;
/// Key derivation: PBKDF2 for master keys, HKDF-SHA256 for domain-separated subkeys.
pub mod key_derivation;
/// Key slots: encrypted containers that hold a content key, unlockable via
/// password, public key (X25519), or dimension key.
pub mod key_slot;
/// Defense-in-depth obfuscation: binary XOR keystream and image color/pixel scrambling.
pub mod obfuscation;
/// Onion encryption for multi-hop relay forwarding (ephemeral X25519 + AES-256-GCM per layer).
pub mod onion;
/// PKCS#7 block padding for fixed-size alignment.
pub mod padding;
/// Password strength estimation with entropy calculation and crack-time estimates.
pub mod password_strength;
/// Deterministic PRNG (xorshift64) for reproducible shuffles. Not cryptographically secure.
pub mod random;
/// BIP-39 recovery phrases: generation, validation, and seed derivation.
pub mod recovery;
/// Memory-safe containers that zero their contents on drop and use constant-time equality.
pub mod secure_data;

// Re-exports for convenience.
pub use encryption::EncryptedData;
pub use error::SentinalError;
pub use key_slot::{
    InternalKeySlot, KeySlot, KeySlotCredential, PasswordKeySlot, PublicKeySlot,
};
pub use onion::{unwrap_layer, wrap_layer};
pub use padding::{pad_to_multiple, unpad_from_multiple};
pub use password_strength::{estimate_strength, PasswordStrength, StrengthTier};
pub use random::SeededRandom;
pub use secure_data::SecureData;
