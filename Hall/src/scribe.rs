//! Scribe — the write path for encrypted .idea packages.
//!
//! Header and Bonds are written as plaintext JSON.
//! Content, Authority, Coinage, and Position are AES-256-GCM encrypted.

use std::path::Path;

use ideas::package::names;
use ideas::IdeaPackage;
use lingo::Babel;

use crate::error::HallError;

/// Encrypted file name constants (plaintext names live in `ideas::package::names`).
pub mod encrypted_names {
    pub const BOOK: &str = "book.encrypted";
    pub const TREE: &str = "tree.encrypted";
    pub const COOL: &str = "value.encrypted";
    pub const REDEMPTION: &str = "redemption.encrypted";
    pub const POSITION: &str = "position.encrypted";
    pub const ASSETS: &str = "Assets";
}

pub use encrypted_names::ASSETS;

/// Write an IdeaPackage to disk with encryption.
///
/// Header and Bonds are plaintext (Header is browseable without a key;
/// Bonds are references, not content). Everything else is AES-256-GCM
/// encrypted via Sentinal.
///
/// When `vocab_seed` is `Some`, JSON content is first Babel-encoded before
/// encryption, adding a streaming cipher layer of semantic obfuscation.
/// When `None`, content is encrypted directly (backward-compatible).
///
/// The destination path is taken from `package.path`.
///
/// Returns total bytes written.
pub fn write(
    package: &IdeaPackage,
    content_key: &[u8],
    vocab_seed: Option<&[u8]>,
) -> Result<u64, HallError> {
    let base = &package.path;
    let mut total: u64 = 0;
    let babel = vocab_seed.map(Babel::new);

    // Create the .idea directory.
    ensure_dir(base)?;

    // 1. Header.json — always plaintext.
    total += write_plaintext_json(base, names::HEADER, &package.header)?;

    // 2. Content/{uuid}.json — each digit encrypted independently.
    let content_dir = base.join(names::CONTENT);
    ensure_dir(&content_dir)?;
    for (id, digit) in &package.digits {
        let filename = format!("{id}.json");
        total += write_encrypted_json(&content_dir, &filename, digit, content_key, babel.as_ref())?;
    }

    // 3. Authority/ — encrypted (optional).
    if package.book.is_some() || package.tree.is_some() {
        let auth_dir = base.join(names::AUTHORITY);
        ensure_dir(&auth_dir)?;
        if let Some(book) = &package.book {
            total += write_encrypted_json(&auth_dir, encrypted_names::BOOK, book, content_key, babel.as_ref())?;
        }
        if let Some(tree) = &package.tree {
            total += write_encrypted_json(&auth_dir, encrypted_names::TREE, tree, content_key, babel.as_ref())?;
        }
    }

    // 4. Coinage/ — encrypted (optional).
    if package.cool.is_some() || package.redemption.is_some() {
        let coin_dir = base.join(names::COINAGE);
        ensure_dir(&coin_dir)?;
        if let Some(cool) = &package.cool {
            total += write_encrypted_json(&coin_dir, encrypted_names::COOL, cool, content_key, babel.as_ref())?;
        }
        if let Some(redemption) = &package.redemption {
            total += write_encrypted_json(
                &coin_dir,
                encrypted_names::REDEMPTION,
                redemption,
                content_key,
                babel.as_ref(),
            )?;
        }
    }

    // 5. Bonds/ — plaintext (references, not content).
    if let Some(bonds) = &package.bonds {
        let bonds_dir = base.join(names::BONDS);
        ensure_dir(&bonds_dir)?;
        if let Some(local) = &bonds.local {
            total += write_plaintext_json(&bonds_dir, names::LOCAL_BONDS, local)?;
        }
        if let Some(priv_bonds) = &bonds.private_bonds {
            total += write_plaintext_json(&bonds_dir, names::PRIVATE_BONDS, priv_bonds)?;
        }
        if let Some(pub_bonds) = &bonds.public_bonds {
            total += write_plaintext_json(&bonds_dir, names::PUBLIC_BONDS, pub_bonds)?;
        }
    }

    // 6. Position/ — encrypted (optional).
    if let Some(position) = &package.position {
        let pos_dir = base.join(names::POSITION);
        ensure_dir(&pos_dir)?;
        total += write_encrypted_json(
            &pos_dir,
            encrypted_names::POSITION,
            position,
            content_key,
            babel.as_ref(),
        )?;
    }

    Ok(total)
}

/// Write a value as pretty-printed JSON. Returns bytes written.
fn write_plaintext_json<T: serde::Serialize>(
    dir: &Path,
    filename: &str,
    data: &T,
) -> Result<u64, HallError> {
    let json = serde_json::to_vec_pretty(data)?;
    let len = json.len() as u64;
    std::fs::write(dir.join(filename), &json)?;
    Ok(len)
}

/// JSON-encode, optionally Babel-encode, then AES-256-GCM encrypt, then write.
///
/// When `babel` is `Some`, the JSON is serialized to a string, Babel-encoded,
/// then the encoded string's bytes are encrypted. When `None`, JSON is
/// serialized directly to bytes and encrypted (original behavior).
///
/// Returns bytes written.
fn write_encrypted_json<T: serde::Serialize>(
    dir: &Path,
    filename: &str,
    data: &T,
    key: &[u8],
    babel: Option<&Babel>,
) -> Result<u64, HallError> {
    let plaintext = match babel {
        Some(b) => {
            let json_str = serde_json::to_string(data)?;
            let encoded = b.encode(&json_str);
            encoded.into_bytes()
        }
        None => serde_json::to_vec(data)?,
    };
    let encrypted = sentinal::encryption::encrypt_combined(&plaintext, key)?;
    let len = encrypted.len() as u64;
    std::fs::write(dir.join(filename), &encrypted)?;
    Ok(len)
}

/// Create a directory (and parents) with a descriptive error on failure.
fn ensure_dir(path: &Path) -> Result<(), HallError> {
    std::fs::create_dir_all(path).map_err(|e| HallError::DirectoryCreation {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ideas::package::names;

    const TEST_KEY: [u8; 32] = [0x42; 32];

    /// Create a minimal test package (header + 1 digit).
    fn minimal_package(dir: &std::path::Path) -> IdeaPackage {
        use chrono::Utc;
        use ideas::digit::Digit;
        use ideas::header::*;
        use uuid::Uuid;
        use x::Value;

        let digit = Digit::new("text".into(), Value::String("hello".into()), "cpub1test".into()).unwrap();
        let root_id = digit.id();
        let header = Header {
            version: "1.0".into(),
            id: Uuid::new_v4(),
            created: Utc::now(),
            modified: Utc::now(),
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
    fn write_header_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);

        write(&package, &TEST_KEY, None).unwrap();

        // Header.json should be readable as JSON.
        let header_path = pkg_path.join(names::HEADER);
        assert!(header_path.exists());
        let raw = std::fs::read_to_string(&header_path).unwrap();
        let _: ideas::Header = serde_json::from_str(&raw).unwrap();
    }

    #[test]
    fn write_encrypted_digit() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);
        let digit_id = package.header.content.root_digit_id;

        write(&package, &TEST_KEY, None).unwrap();

        // Content/{uuid}.json should exist but NOT be valid JSON.
        let digit_path = pkg_path
            .join(names::CONTENT)
            .join(format!("{digit_id}.json"));
        assert!(digit_path.exists());
        let raw = std::fs::read(&digit_path).unwrap();
        let parse_result = serde_json::from_slice::<serde_json::Value>(&raw);
        assert!(parse_result.is_err(), "encrypted bytes should not be valid JSON");
    }

    #[test]
    fn write_encrypted_authority() {
        use ideas::authority::{Book, Tree};
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path)
            .with_book(Book::new("cpub1creator".into(), "sig_test".into()))
            .with_tree(Tree::new());

        write(&package, &TEST_KEY, None).unwrap();

        let auth_dir = pkg_path.join(names::AUTHORITY);
        assert!(auth_dir.join(encrypted_names::BOOK).exists());
        assert!(auth_dir.join(encrypted_names::TREE).exists());
    }

    #[test]
    fn write_plaintext_bonds() {
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

        write(&package, &TEST_KEY, None).unwrap();

        // Bonds should be readable as plaintext JSON.
        let local_path = pkg_path.join(names::BONDS).join(names::LOCAL_BONDS);
        assert!(local_path.exists());
        let raw = std::fs::read_to_string(&local_path).unwrap();
        let _: ideas::bonds::LocalBonds = serde_json::from_str(&raw).unwrap();
    }

    #[test]
    fn write_encrypted_coinage() {
        use ideas::coinage::Cool;
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path).with_cool(Cool::new(100));

        write(&package, &TEST_KEY, None).unwrap();

        let cool_path = pkg_path.join(names::COINAGE).join(encrypted_names::COOL);
        assert!(cool_path.exists());
        // Should be encrypted, not JSON.
        let raw = std::fs::read(&cool_path).unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&raw).is_err());
    }

    #[test]
    fn write_encrypted_position() {
        use ideas::position::Position;
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path)
            .with_position(Position::new(
                ideas::position::Coordinates { x: 1.0, y: 2.0, z: 3.0 },
                false,
            ));

        write(&package, &TEST_KEY, None).unwrap();

        let pos_path = pkg_path
            .join(names::POSITION)
            .join(encrypted_names::POSITION);
        assert!(pos_path.exists());
    }

    #[test]
    fn skip_empty_optional_sections() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);

        write(&package, &TEST_KEY, None).unwrap();

        // No optional dirs should exist.
        assert!(!pkg_path.join(names::AUTHORITY).exists());
        assert!(!pkg_path.join(names::BONDS).exists());
        assert!(!pkg_path.join(names::COINAGE).exists());
        assert!(!pkg_path.join(names::POSITION).exists());
    }

    #[test]
    fn bytes_written_is_accurate() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_path = dir.path().join("test.idea");
        let package = minimal_package(&pkg_path);

        let bytes = write(&package, &TEST_KEY, None).unwrap();
        assert!(bytes > 0);

        // Sum all file sizes.
        let mut actual: u64 = 0;
        for entry in walkdir(&pkg_path) {
            if entry.is_file() {
                actual += std::fs::metadata(&entry).unwrap().len();
            }
        }
        assert_eq!(bytes, actual);
    }

    /// Recursively collect all file paths under a directory.
    fn walkdir(path: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let p = entry.path();
                if p.is_dir() {
                    files.extend(walkdir(&p));
                } else {
                    files.push(p);
                }
            }
        }
        files
    }
}
