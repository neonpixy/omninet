//! Scholar — the read path for encrypted .idea packages.
//!
//! Graceful degradation: a corrupted digit produces a warning, not a fatal
//! error. Only a missing or corrupted header is fatal. This preserves the
//! Covenant's Dignity principle — corrupted content doesn't destroy the
//! whole idea.

use std::collections::HashMap;
use std::path::Path;

use ideas::authority::{Book, Tree};
use ideas::bonds::{Bonds, LocalBonds, PrivateBonds, PublicBonds};
use ideas::coinage::{Cool, Redemption};
use ideas::digit::Digit;
use ideas::package::names;
use ideas::position::Position;
use ideas::Header;
use ideas::IdeaPackage;
use lingo::Babel;
use uuid::Uuid;

use crate::error::{HallError, HallWarning, ReadResult};
use crate::scribe::encrypted_names;

/// Read just the header from an .idea package (no key needed).
///
/// Sovereignty: you can browse your own library without decryption.
pub fn read_header(path: &Path) -> Result<Header, HallError> {
    let header_path = path.join(names::HEADER);
    if !header_path.exists() {
        return Err(HallError::MissingHeader(path.to_path_buf()));
    }
    let data = std::fs::read_to_string(&header_path)?;
    serde_json::from_str(&data).map_err(|e| HallError::CorruptedHeader(e.to_string()))
}

/// Read a full .idea package with decryption and graceful degradation.
///
/// Only a missing/corrupted header is a fatal error. Everything else
/// (digits, authority, coinage, bonds, position) produces warnings on
/// failure and continues loading what it can.
///
/// When `vocab_seed` is `Some`, decrypted content is treated as Babel-encoded
/// Unicode symbols and decoded before JSON deserialization. When `None`,
/// decrypted content is deserialized directly (backward-compatible).
pub fn read(
    path: &Path,
    content_key: &[u8],
    vocab_seed: Option<&[u8]>,
) -> Result<ReadResult<IdeaPackage>, HallError> {
    if !path.is_dir() {
        return Err(HallError::NotAnIdeaPackage(path.to_path_buf()));
    }

    let mut warnings: Vec<HallWarning> = Vec::new();
    let babel = vocab_seed.map(Babel::new);

    // 1. Header — fatal on failure.
    let header = read_header(path)?;

    // 2. Content/ — each digit independently, graceful.
    let digits = read_digits(path, content_key, babel.as_ref(), &mut warnings);

    // 3. Authority/ — optional, graceful.
    let book: Option<Book> = try_read_encrypted(
        &path.join(names::AUTHORITY).join(encrypted_names::BOOK),
        content_key,
        babel.as_ref(),
        &mut warnings,
        "authority/book",
    );
    let tree: Option<Tree> = try_read_encrypted(
        &path.join(names::AUTHORITY).join(encrypted_names::TREE),
        content_key,
        babel.as_ref(),
        &mut warnings,
        "authority/tree",
    );

    // 4. Coinage/ — optional, graceful.
    let cool: Option<Cool> = try_read_encrypted(
        &path.join(names::COINAGE).join(encrypted_names::COOL),
        content_key,
        babel.as_ref(),
        &mut warnings,
        "coinage/value",
    );
    let redemption: Option<Redemption> = try_read_encrypted(
        &path.join(names::COINAGE).join(encrypted_names::REDEMPTION),
        content_key,
        babel.as_ref(),
        &mut warnings,
        "coinage/redemption",
    );

    // 5. Bonds/ — plaintext, optional, graceful.
    let local: Option<LocalBonds> = try_read_plaintext(
        &path.join(names::BONDS).join(names::LOCAL_BONDS),
        &mut warnings,
        "bonds/local",
    );
    let private_bonds: Option<PrivateBonds> = try_read_plaintext(
        &path.join(names::BONDS).join(names::PRIVATE_BONDS),
        &mut warnings,
        "bonds/private",
    );
    let public_bonds: Option<PublicBonds> = try_read_plaintext(
        &path.join(names::BONDS).join(names::PUBLIC_BONDS),
        &mut warnings,
        "bonds/public",
    );
    let bonds = if local.is_some() || private_bonds.is_some() || public_bonds.is_some() {
        Some(Bonds {
            local,
            private_bonds,
            public_bonds,
        })
    } else {
        None
    };

    // 6. Position/ — encrypted, optional, graceful.
    let position: Option<Position> = try_read_encrypted(
        &path.join(names::POSITION).join(encrypted_names::POSITION),
        content_key,
        babel.as_ref(),
        &mut warnings,
        "position",
    );

    // Assemble the package.
    let package = IdeaPackage {
        path: path.to_path_buf(),
        header,
        digits,
        book,
        tree,
        cool,
        redemption,
        bonds,
        position,
    };

    Ok(ReadResult::with_warnings(package, warnings))
}

/// Check whether a path is a .idea package (has Header.json).
pub fn is_idea_package(path: &Path) -> bool {
    path.is_dir() && path.join(names::HEADER).exists()
}

// ── Private helpers ──────────────────────────────────────────────────

/// Read all digits from Content/, each independently decrypted.
fn read_digits(
    path: &Path,
    key: &[u8],
    babel: Option<&Babel>,
    warnings: &mut Vec<HallWarning>,
) -> HashMap<Uuid, Digit> {
    let mut digits = HashMap::new();
    let content_dir = path.join(names::CONTENT);

    if !content_dir.is_dir() {
        return digits;
    }

    let entries = match std::fs::read_dir(&content_dir) {
        Ok(e) => e,
        Err(e) => {
            warnings.push(HallWarning::with_file(
                format!("cannot read Content directory: {e}"),
                content_dir.display().to_string(),
            ));
            return digits;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warnings.push(HallWarning::new(format!("directory entry error: {e}")));
                continue;
            }
        };

        let file_path = entry.path();

        // Only process .json files.
        if file_path.extension().is_none_or(|e| e != "json") {
            continue;
        }

        // Parse UUID from filename stem.
        let stem = match file_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => {
                warnings.push(HallWarning::with_file(
                    "invalid filename",
                    file_path.display().to_string(),
                ));
                continue;
            }
        };

        let id = match Uuid::parse_str(stem) {
            Ok(id) => id,
            Err(_) => {
                warnings.push(HallWarning::with_file(
                    format!("cannot parse UUID from filename: {stem}"),
                    file_path.display().to_string(),
                ));
                continue;
            }
        };

        // Read + decrypt + (optionally Babel-decode) + deserialize.
        match read_encrypted_json::<Digit>(&file_path, key, babel) {
            Ok(digit) => {
                digits.insert(digit.id(), digit);
            }
            Err(e) => {
                warnings.push(HallWarning::with_file(
                    format!("corrupted digit {id}: {e}"),
                    file_path.display().to_string(),
                ));
            }
        }
    }

    digits
}

/// Try to read and decrypt a JSON file. If the file doesn't exist, return None.
/// If the file exists but fails, push a warning and return None.
fn try_read_encrypted<T: serde::de::DeserializeOwned>(
    path: &Path,
    key: &[u8],
    babel: Option<&Babel>,
    warnings: &mut Vec<HallWarning>,
    section_name: &str,
) -> Option<T> {
    if !path.exists() {
        return None;
    }
    match read_encrypted_json(path, key, babel) {
        Ok(val) => Some(val),
        Err(e) => {
            warnings.push(HallWarning::with_file(
                format!("corrupted {section_name}: {e}"),
                path.display().to_string(),
            ));
            None
        }
    }
}

/// Try to read a plaintext JSON file. If it doesn't exist, return None.
/// If it exists but fails, push a warning and return None.
fn try_read_plaintext<T: serde::de::DeserializeOwned>(
    path: &Path,
    warnings: &mut Vec<HallWarning>,
    section_name: &str,
) -> Option<T> {
    if !path.exists() {
        return None;
    }
    match read_plaintext_json(path) {
        Ok(val) => Some(val),
        Err(e) => {
            warnings.push(HallWarning::with_file(
                format!("corrupted {section_name}: {e}"),
                path.display().to_string(),
            ));
            None
        }
    }
}

/// Read and deserialize a plaintext JSON file.
fn read_plaintext_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, HallError> {
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Read, decrypt (AES-256-GCM combined format), optionally Babel-decode, and
/// deserialize a JSON file.
///
/// When `babel` is `Some`, the decrypted bytes are interpreted as a UTF-8
/// Babel-encoded string, decoded back to JSON, then deserialized. When `None`,
/// the decrypted bytes are deserialized directly (original behavior).
fn read_encrypted_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    key: &[u8],
    babel: Option<&Babel>,
) -> Result<T, HallError> {
    let encrypted = std::fs::read(path)?;
    let plaintext = sentinal::encryption::decrypt_combined(&encrypted, key)?;
    match babel {
        Some(b) => {
            // Auto-detect: JSON always starts with ASCII (byte < 128: '{', '[', '"', etc.).
            // Babel-encoded text starts with Unicode symbols (multi-byte UTF-8, first byte > 127).
            // This lets us read both old (pre-Babel) and new (Babel-encoded) content seamlessly.
            let is_babel_encoded = plaintext.first().is_some_and(|&b| b > 127);
            if is_babel_encoded {
                let encoded_str = String::from_utf8(plaintext).map_err(|e| {
                    HallError::BabelDecodeFailed(format!("decrypted data is not valid UTF-8: {e}"))
                })?;
                let json_str = b.decode(&encoded_str);
                Ok(serde_json::from_str(&json_str)?)
            } else {
                // Pre-Babel content — decrypt was enough, deserialize directly.
                Ok(serde_json::from_slice(&plaintext)?)
            }
        }
        None => Ok(serde_json::from_slice(&plaintext)?),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scribe;
    use ideas::header::*;
    use x::Value;

    const TEST_KEY: [u8; 32] = [0x42; 32];
    const WRONG_KEY: [u8; 32] = [0x43; 32];
    const TEST_VOCAB_SEED: &[u8] = b"omnidea-babel-test-seed-00000032";

    fn minimal_package(dir: &std::path::Path) -> IdeaPackage {
        use ideas::digit::Digit;

        let digit =
            Digit::new("text".into(), Value::String("hello".into()), "cpub1test".into()).unwrap();
        let root_id = digit.id();
        let header = Header {
            version: "1.0".into(),
            id: uuid::Uuid::new_v4(),
            created: chrono::Utc::now(),
            modified: chrono::Utc::now(),
            extended_type: Some("text".into()),
            creator: Creator {
                public_key: "cpub1test".into(),
                signature: "sig_placeholder".into(),
            },
            content: ContentMetadata {
                root_digit_id: root_id,
                digit_count: 1,
                types: vec!["text".into()],
            },
            encryption: EncryptionConfig {
                algorithm: "AES-256-GCM".into(),
                key_slots: vec![],
            },
            babel: BabelConfig {
                enabled: false,
                vocabulary_seed: None,
                translation_kit: None,
            },
        };
        IdeaPackage::new(dir.to_path_buf(), header, digit)
    }

    #[test]
    fn read_header_from_valid_package() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);
        scribe::write(&package, &TEST_KEY, None).unwrap();

        let header = read_header(&pkg_path).unwrap();
        assert_eq!(header.id, package.header.id);
        assert_eq!(header.version, "1.0");
    }

    #[test]
    fn read_header_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("nonexistent.idea");
        let result = read_header(&pkg_path);
        assert!(matches!(result, Err(HallError::MissingHeader(_))));
    }

    #[test]
    fn read_header_corrupted_json() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        std::fs::create_dir_all(&pkg_path).unwrap();
        std::fs::write(pkg_path.join(names::HEADER), "not valid json!!!").unwrap();

        let result = read_header(&pkg_path);
        assert!(matches!(result, Err(HallError::CorruptedHeader(_))));
    }

    #[test]
    fn read_full_package_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);
        let original_id = package.header.id;
        let root_digit_id = package.header.content.root_digit_id;

        scribe::write(&package, &TEST_KEY, None).unwrap();
        let result = read(&pkg_path, &TEST_KEY, None).unwrap();

        assert!(!result.has_warnings());
        assert_eq!(result.value.header.id, original_id);
        assert!(result.value.digits.contains_key(&root_digit_id));
    }

    #[test]
    fn corrupted_digit_produces_warning() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");

        // Write a package with 2 digits.
        let digit2 =
            ideas::digit::Digit::new("text".into(), Value::String("world".into()), "cpub1test".into())
                .unwrap();
        let digit2_id = digit2.id();
        let package = minimal_package(&pkg_path).with_digit(digit2);
        let root_id = package.header.content.root_digit_id;

        scribe::write(&package, &TEST_KEY, None).unwrap();

        // Corrupt one digit file.
        let corrupted_path = pkg_path
            .join(names::CONTENT)
            .join(format!("{digit2_id}.json"));
        std::fs::write(&corrupted_path, b"garbage data").unwrap();

        let result = read(&pkg_path, &TEST_KEY, None).unwrap();
        assert!(result.has_warnings());
        assert_eq!(result.warnings.len(), 1);
        // The uncorrupted digit should still be loaded.
        assert!(result.value.digits.contains_key(&root_id));
        // The corrupted digit should be missing.
        assert!(!result.value.digits.contains_key(&digit2_id));
    }

    #[test]
    fn missing_optional_sections_no_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);

        scribe::write(&package, &TEST_KEY, None).unwrap();
        let result = read(&pkg_path, &TEST_KEY, None).unwrap();

        assert!(!result.has_warnings());
        assert!(result.value.book.is_none());
        assert!(result.value.tree.is_none());
        assert!(result.value.cool.is_none());
        assert!(result.value.redemption.is_none());
        assert!(result.value.bonds.is_none());
        assert!(result.value.position.is_none());
    }

    #[test]
    fn read_with_wrong_key_graceful() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);

        scribe::write(&package, &TEST_KEY, None).unwrap();

        // Wrong key: header loads (plaintext) but digits fail.
        let result = read(&pkg_path, &WRONG_KEY, None).unwrap();
        assert!(result.has_warnings());
        assert!(result.value.digits.is_empty());
        // Header should still be correct.
        assert_eq!(result.value.header.id, package.header.id);
    }

    #[test]
    fn read_encrypted_authority() {
        use ideas::authority::{Book, Tree};
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path)
            .with_book(Book::new("cpub1creator".into(), "sig_test".into()))
            .with_tree(Tree::new());

        scribe::write(&package, &TEST_KEY, None).unwrap();
        let result = read(&pkg_path, &TEST_KEY, None).unwrap();

        assert!(!result.has_warnings());
        assert!(result.value.book.is_some());
        assert!(result.value.tree.is_some());
    }

    #[test]
    fn read_plaintext_bonds() {
        use ideas::bonds::{BondRelationship, Bonds, LocalBondReference, LocalBonds};
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");

        let bonds = Bonds {
            local: Some(LocalBonds {
                references: vec![LocalBondReference {
                    idea_id: uuid::Uuid::new_v4(),
                    path: "/test/other.idea".into(),
                    relationship: BondRelationship::Related,
                    verified: false,
                    last_verified: None,
                }],
            }),
            private_bonds: None,
            public_bonds: None,
        };
        let package = minimal_package(&pkg_path).with_bonds(bonds);

        scribe::write(&package, &TEST_KEY, None).unwrap();
        let result = read(&pkg_path, &TEST_KEY, None).unwrap();

        assert!(!result.has_warnings());
        assert!(result.value.bonds.is_some());
        assert_eq!(result.value.bonds.unwrap().count(), 1);
    }

    #[test]
    fn is_idea_package_checks() {
        let dir = tempfile::tempdir().unwrap();

        // Not a package (no Header.json).
        assert!(!is_idea_package(dir.path()));

        // Create a valid package.
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);
        scribe::write(&package, &TEST_KEY, None).unwrap();
        assert!(is_idea_package(&pkg_path));

        // File, not directory.
        let file_path = dir.path().join("not_a_dir.idea");
        std::fs::write(&file_path, "hello").unwrap();
        assert!(!is_idea_package(&file_path));
    }

    // ── Babel integration tests ─────────────────────────────────────────

    #[test]
    fn write_read_with_babel_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("babel.idea");
        let package = minimal_package(&pkg_path);
        let original_id = package.header.id;
        let root_digit_id = package.header.content.root_digit_id;

        scribe::write(&package, &TEST_KEY, Some(TEST_VOCAB_SEED)).unwrap();
        let result = read(&pkg_path, &TEST_KEY, Some(TEST_VOCAB_SEED)).unwrap();

        assert!(!result.has_warnings(), "warnings: {:?}", result.warnings);
        assert_eq!(result.value.header.id, original_id);
        assert!(result.value.digits.contains_key(&root_digit_id));
        let digit = &result.value.digits[&root_digit_id];
        assert_eq!(digit.content, Value::String("hello".into()));
    }

    #[test]
    fn babel_none_backward_compatible() {
        // Write without Babel, read without Babel — existing behavior preserved.
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("compat.idea");
        let package = minimal_package(&pkg_path);
        let root_digit_id = package.header.content.root_digit_id;

        scribe::write(&package, &TEST_KEY, None).unwrap();
        let result = read(&pkg_path, &TEST_KEY, None).unwrap();

        assert!(!result.has_warnings());
        assert!(result.value.digits.contains_key(&root_digit_id));
        let digit = &result.value.digits[&root_digit_id];
        assert_eq!(digit.content, Value::String("hello".into()));
    }

    #[test]
    fn babel_encoded_not_readable_as_json() {
        // Write with Babel. Raw decrypted bytes should NOT be valid JSON
        // (they're Babel-encoded Unicode symbols).
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("opaque.idea");
        let package = minimal_package(&pkg_path);
        let digit_id = package.header.content.root_digit_id;

        scribe::write(&package, &TEST_KEY, Some(TEST_VOCAB_SEED)).unwrap();

        // Manually decrypt the digit file and check it's not valid JSON.
        let digit_path = pkg_path
            .join(names::CONTENT)
            .join(format!("{digit_id}.json"));
        let encrypted = std::fs::read(&digit_path).unwrap();
        let decrypted = sentinal::encryption::decrypt_combined(&encrypted, &TEST_KEY).unwrap();
        let decrypted_str = String::from_utf8(decrypted).unwrap();

        // The decrypted string is Babel symbols, not JSON.
        let parse_result = serde_json::from_str::<serde_json::Value>(&decrypted_str);
        assert!(
            parse_result.is_err(),
            "Babel-encoded content should not be valid JSON, got: {decrypted_str}"
        );
    }

    #[test]
    fn babel_wrong_seed_fails() {
        // Write with one seed, read with a different seed — content should be
        // garbled (deserialization will fail, producing warnings).
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("wrong_seed.idea");
        let package = minimal_package(&pkg_path);

        let other_seed: &[u8] = b"different-babel-seed-for-testing!";

        scribe::write(&package, &TEST_KEY, Some(TEST_VOCAB_SEED)).unwrap();
        let result = read(&pkg_path, &TEST_KEY, Some(other_seed)).unwrap();

        // Header still loads (plaintext), but digits should fail
        // because Babel decodes to garbled text, not valid JSON.
        assert!(result.has_warnings(), "expected warnings from wrong Babel seed");
        assert!(
            result.value.digits.is_empty(),
            "digits should be empty when decoded with wrong Babel seed"
        );
    }
}
