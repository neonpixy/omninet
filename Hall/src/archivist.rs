//! Archivist — binary asset pipeline for .idea packages.
//!
//! Three layers of protection:
//! 1. **SHA-256 content addressing** — filename IS the hash (deduplication + integrity)
//! 2. **Babel obfuscation** — XOR with HMAC-SHA256 keystream (defense in depth)
//! 3. **AES-256-GCM encryption** — authenticated encryption
//!
//! Import: `data → SHA-256(data) → obfuscate(data, seed) → encrypt(obfuscated, key) → Assets/{hash}.shuffled`
//! Read:   `read .shuffled → decrypt(key) → deobfuscate(seed) → SHA-256 verify → data`

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::HallError;
use crate::scribe::encrypted_names;

const SHUFFLED_EXTENSION: &str = "shuffled";

/// Import raw bytes as an encrypted asset. Returns the SHA-256 hex hash.
///
/// The asset is hashed, obfuscated with Babel, encrypted with AES-256-GCM,
/// and written to `Assets/{hash}.shuffled`.
pub fn import(
    data: &[u8],
    idea_path: &Path,
    content_key: &[u8],
    vocab_seed: &[u8],
) -> Result<String, HallError> {
    // 1. Hash the original plaintext.
    let hash_hex = sha256_hex(data);

    // 2. Obfuscate with Babel XOR keystream.
    let obfuscated = sentinal::obfuscation::obfuscate(data, vocab_seed);

    // 3. Encrypt.
    let encrypted = sentinal::encryption::encrypt_combined(&obfuscated, content_key)?;

    // 4. Write to Assets/{hash}.shuffled.
    let assets_dir = idea_path.join(encrypted_names::ASSETS);
    std::fs::create_dir_all(&assets_dir).map_err(|e| HallError::DirectoryCreation {
        path: assets_dir.clone(),
        source: e,
    })?;

    let asset_path = assets_dir.join(format!("{hash_hex}.{SHUFFLED_EXTENSION}"));
    std::fs::write(&asset_path, &encrypted)?;

    Ok(hash_hex)
}

/// Import a file as an encrypted asset. Returns the SHA-256 hex hash.
pub fn import_file(
    source: &Path,
    idea_path: &Path,
    content_key: &[u8],
    vocab_seed: &[u8],
) -> Result<String, HallError> {
    let data = std::fs::read(source)?;
    import(&data, idea_path, content_key, vocab_seed)
}

/// Read an asset by its hash. Decrypts, deobfuscates, and verifies integrity.
///
/// Hash verification is MANDATORY — catches both corruption and tampering.
pub fn read(
    hash: &str,
    idea_path: &Path,
    content_key: &[u8],
    vocab_seed: &[u8],
) -> Result<Vec<u8>, HallError> {
    let asset_path = idea_path
        .join(encrypted_names::ASSETS)
        .join(format!("{hash}.{SHUFFLED_EXTENSION}"));

    // 1. Read encrypted bytes.
    let encrypted = std::fs::read(&asset_path).map_err(|e| HallError::CorruptedAsset {
        hash: hash.to_string(),
        reason: format!("read failed: {e}"),
    })?;

    // 2. Decrypt.
    let obfuscated =
        sentinal::encryption::decrypt_combined(&encrypted, content_key).map_err(|e| {
            HallError::CorruptedAsset {
                hash: hash.to_string(),
                reason: format!("decryption failed: {e}"),
            }
        })?;

    // 3. Deobfuscate.
    let plaintext = sentinal::obfuscation::deobfuscate(&obfuscated, vocab_seed);

    // 4. MANDATORY hash verification.
    let computed = sha256_hex(&plaintext);
    if computed != hash {
        return Err(HallError::AssetHashMismatch {
            expected: hash.to_string(),
            actual: computed,
        });
    }

    Ok(plaintext)
}

/// Export an asset to a destination file.
pub fn export(
    hash: &str,
    idea_path: &Path,
    dest: &Path,
    content_key: &[u8],
    vocab_seed: &[u8],
) -> Result<(), HallError> {
    let data = read(hash, idea_path, content_key, vocab_seed)?;
    std::fs::write(dest, &data)?;
    Ok(())
}

/// List all asset hashes in the Assets/ directory.
pub fn list(idea_path: &Path) -> Result<Vec<String>, HallError> {
    let assets_dir = idea_path.join(encrypted_names::ASSETS);
    if !assets_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut hashes = Vec::new();
    for entry in std::fs::read_dir(&assets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == SHUFFLED_EXTENSION) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                hashes.push(stem.to_string());
            }
        }
    }
    hashes.sort();
    Ok(hashes)
}

/// Check if an asset exists by hash.
pub fn exists(hash: &str, idea_path: &Path) -> bool {
    idea_path
        .join(encrypted_names::ASSETS)
        .join(format!("{hash}.{SHUFFLED_EXTENSION}"))
        .exists()
}

/// Delete an asset by hash.
pub fn delete(hash: &str, idea_path: &Path) -> Result<(), HallError> {
    let path = idea_path
        .join(encrypted_names::ASSETS)
        .join(format!("{hash}.{SHUFFLED_EXTENSION}"));
    std::fs::remove_file(&path)?;
    Ok(())
}

/// Compute SHA-256 hash and return as lowercase hex string.
fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: [u8; 32] = [0x42; 32];
    const VOCAB_SEED: [u8; 32] = [0x99; 32];

    fn setup_idea_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let idea_path = dir.path().join("test.idea");
        std::fs::create_dir_all(&idea_path).unwrap();
        (dir, idea_path)
    }

    #[test]
    fn import_returns_hex_hash() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"sovereign binary data";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        // SHA-256 hex is 64 characters.
        assert_eq!(hash.len(), 64);
        // File should exist.
        assert!(exists(&hash, &idea_path));
    }

    #[test]
    fn import_file_same_hash_as_raw() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"test file contents";

        // Import from raw bytes.
        let hash1 = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        // Write to a temp file and import.
        let file_path = idea_path.parent().unwrap().join("source.bin");
        std::fs::write(&file_path, data).unwrap();

        // Need a different idea path to avoid collision.
        let idea_path2 = idea_path.parent().unwrap().join("test2.idea");
        std::fs::create_dir_all(&idea_path2).unwrap();
        let hash2 = import_file(&file_path, &idea_path2, &TEST_KEY, &VOCAB_SEED).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn read_asset_round_trip() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"round trip through the Archivist";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        let recovered = read(&hash, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn hash_verification_catches_wrong_seed() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"important data";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        // Read with wrong vocabulary seed — deobfuscation produces wrong bytes,
        // hash won't match.
        let wrong_seed = [0xAA; 32];
        let result = read(&hash, &idea_path, &TEST_KEY, &wrong_seed);
        assert!(matches!(result, Err(HallError::AssetHashMismatch { .. })));
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"encrypted asset";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        let wrong_key = [0x43; 32];
        let result = read(&hash, &idea_path, &wrong_key, &VOCAB_SEED);
        assert!(matches!(result, Err(HallError::CorruptedAsset { .. })));
    }

    #[test]
    fn list_assets_returns_all() {
        let (_dir, idea_path) = setup_idea_dir();

        assert!(list(&idea_path).unwrap().is_empty());

        let hash1 = import(b"asset one", &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();
        let hash2 = import(b"asset two", &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        let listed = list(&idea_path).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.contains(&hash1));
        assert!(listed.contains(&hash2));
    }

    #[test]
    fn exists_and_delete() {
        let (_dir, idea_path) = setup_idea_dir();
        let hash = import(b"to be deleted", &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        assert!(exists(&hash, &idea_path));
        delete(&hash, &idea_path).unwrap();
        assert!(!exists(&hash, &idea_path));
    }

    #[test]
    fn export_writes_decrypted_file() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"export me";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        let dest = idea_path.parent().unwrap().join("exported.bin");
        export(&hash, &idea_path, &dest, &TEST_KEY, &VOCAB_SEED).unwrap();

        let exported = std::fs::read(&dest).unwrap();
        assert_eq!(exported, data);
    }

    #[test]
    fn encrypted_data_is_not_plaintext() {
        let (_dir, idea_path) = setup_idea_dir();
        let data = b"this should be unrecognizable on disk";
        let hash = import(data, &idea_path, &TEST_KEY, &VOCAB_SEED).unwrap();

        let asset_path = idea_path
            .join(encrypted_names::ASSETS)
            .join(format!("{hash}.{SHUFFLED_EXTENSION}"));
        let raw = std::fs::read(&asset_path).unwrap();

        // Raw bytes should NOT contain the plaintext.
        let found = raw
            .windows(data.len())
            .any(|window| window == data);
        assert!(!found, "plaintext should not appear in encrypted asset");
    }
}
