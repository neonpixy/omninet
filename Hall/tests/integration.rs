use chrono::Utc;
use uuid::Uuid;
use x::Value;

use ideas::authority::{Book, Tree};
use ideas::bonds::{BondRelationship, Bonds, LocalBondReference, LocalBonds};
use ideas::coinage::Cool;
use ideas::digit::Digit;
use ideas::header::*;
use ideas::position::{Coordinates, Position};
use ideas::IdeaPackage;

use hall::{archivist, scholar, scribe};

const KEY: [u8; 32] = [0x42; 32];
const WRONG_KEY: [u8; 32] = [0x43; 32];
const VOCAB_SEED: [u8; 32] = [0x99; 32];

fn full_package(dir: &std::path::Path) -> IdeaPackage {
    let digit1 = Digit::new(
        "text".into(),
        Value::String("Chapter one".into()),
        "cpub1author".into(),
    )
    .unwrap();
    let digit2 = Digit::new(
        "image".into(),
        Value::String("photo.png".into()),
        "cpub1author".into(),
    )
    .unwrap();
    let root_id = digit1.id();

    let header = Header {
        version: "1.0".into(),
        id: Uuid::new_v4(),
        created: Utc::now(),
        modified: Utc::now(),
        extended_type: Some("document".into()),
        creator: Creator {
            public_key: "cpub1author".into(),
            signature: "sig_test".into(),
        },
        content: ContentMetadata {
            root_digit_id: root_id,
            digit_count: 2,
            types: vec!["text".into(), "image".into()],
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

    let bonds = Bonds {
        local: Some(LocalBonds {
            references: vec![LocalBondReference {
                idea_id: Uuid::new_v4(),
                path: "/home/user/other.idea".into(),
                relationship: BondRelationship::Related,
                verified: false,
                last_verified: None,
            }],
        }),
        private_bonds: None,
        public_bonds: None,
    };

    IdeaPackage::new(dir.to_path_buf(), header, digit1)
        .with_digit(digit2)
        .with_book(Book::new("cpub1author".into(), "sig_book".into()))
        .with_tree(Tree::new())
        .with_cool(Cool::new(100_000))
        .with_bonds(bonds)
        .with_position(Position::new(
            Coordinates {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            },
            true,
        ))
}

#[test]
fn full_write_read_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("complete.idea");
    let original = full_package(&pkg_path);
    let original_id = original.header.id;
    let root_digit_id = original.header.content.root_digit_id;

    scribe::write(&original, &KEY, None).unwrap();
    let result = scholar::read(&pkg_path, &KEY, None).unwrap();

    assert!(!result.has_warnings());
    let loaded = &result.value;

    // Header matches.
    assert_eq!(loaded.header.id, original_id);
    assert_eq!(loaded.header.version, "1.0");
    assert_eq!(loaded.header.extended_type.as_deref(), Some("document"));

    // Digits match (both loaded).
    assert_eq!(loaded.digits.len(), 2);
    assert!(loaded.digits.contains_key(&root_digit_id));

    // Authority present.
    assert!(loaded.book.is_some());
    assert!(loaded.tree.is_some());

    // Coinage present.
    assert!(loaded.cool.is_some());

    // Bonds present (plaintext).
    assert!(loaded.bonds.is_some());
    assert_eq!(loaded.bonds.as_ref().unwrap().count(), 1);

    // Position present.
    assert!(loaded.position.is_some());
    assert!(loaded.position.as_ref().unwrap().pinned);
}

#[test]
fn encrypted_content_not_readable_as_plaintext() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("test.idea");
    let package = full_package(&pkg_path);
    let root_id = package.header.content.root_digit_id;

    scribe::write(&package, &KEY, None).unwrap();

    // Try to parse an encrypted digit file as JSON.
    let digit_path = pkg_path
        .join("Content")
        .join(format!("{root_id}.json"));
    let raw = std::fs::read(&digit_path).unwrap();
    let parse = serde_json::from_slice::<serde_json::Value>(&raw);
    assert!(parse.is_err(), "encrypted bytes must not be valid JSON");
}

#[test]
fn wrong_key_graceful_degradation() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("test.idea");
    let package = full_package(&pkg_path);

    scribe::write(&package, &KEY, None).unwrap();

    // Read with wrong key — header loads (plaintext) but everything
    // encrypted fails gracefully with warnings.
    let result = scholar::read(&pkg_path, &WRONG_KEY, None).unwrap();
    assert!(result.has_warnings());

    // Header is correct (plaintext).
    assert_eq!(result.value.header.id, package.header.id);

    // All encrypted sections should have failed.
    assert!(result.value.digits.is_empty());
    assert!(result.value.book.is_none());
    assert!(result.value.tree.is_none());
    assert!(result.value.cool.is_none());
    assert!(result.value.position.is_none());

    // Bonds are plaintext, so they should still load.
    assert!(result.value.bonds.is_some());
}

#[test]
fn header_readable_without_key() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("test.idea");
    let package = full_package(&pkg_path);

    scribe::write(&package, &KEY, None).unwrap();

    // read_header takes no key parameter.
    let header = scholar::read_header(&pkg_path).unwrap();
    assert_eq!(header.id, package.header.id);
    assert_eq!(header.creator.public_key, "cpub1author");
}

#[test]
fn corrupted_digit_loads_others() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("test.idea");
    let package = full_package(&pkg_path);
    let root_id = package.header.content.root_digit_id;

    scribe::write(&package, &KEY, None).unwrap();

    // Find a digit that isn't the root and corrupt it.
    let content_dir = pkg_path.join("Content");
    let mut corrupted_one = false;
    for entry in std::fs::read_dir(&content_dir).unwrap() {
        let entry = entry.unwrap();
        let stem = entry.path().file_stem().unwrap().to_str().unwrap().to_string();
        let id = uuid::Uuid::parse_str(&stem).unwrap();
        if id != root_id && !corrupted_one {
            std::fs::write(entry.path(), b"corruption").unwrap();
            corrupted_one = true;
        }
    }
    assert!(corrupted_one);

    let result = scholar::read(&pkg_path, &KEY, None).unwrap();
    assert!(result.has_warnings());
    assert_eq!(result.value.digits.len(), 1);
    assert!(result.value.digits.contains_key(&root_id));
}

#[test]
fn asset_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let idea_path = dir.path().join("test.idea");
    std::fs::create_dir_all(&idea_path).unwrap();

    let data = b"A sovereign binary asset, protected by the Covenant.";
    let hash = archivist::import(data, &idea_path, &KEY, &VOCAB_SEED).unwrap();

    let recovered = archivist::read(&hash, &idea_path, &KEY, &VOCAB_SEED).unwrap();
    assert_eq!(recovered, data);

    // List shows it.
    let listed = archivist::list(&idea_path).unwrap();
    assert_eq!(listed, vec![hash.clone()]);

    // Exists check.
    assert!(archivist::exists(&hash, &idea_path));
}

#[test]
fn asset_hash_mismatch_detected() {
    let dir = tempfile::tempdir().unwrap();
    let idea_path = dir.path().join("test.idea");
    std::fs::create_dir_all(&idea_path).unwrap();

    let data = b"original data";
    let hash = archivist::import(data, &idea_path, &KEY, &VOCAB_SEED).unwrap();

    // Tamper with the .shuffled file — overwrite with valid encrypted data
    // of DIFFERENT content (same key, so decryption succeeds but hash won't match).
    let different_data = b"tampered data!!!";
    let obfuscated = sentinal::obfuscation::obfuscate(different_data, &VOCAB_SEED);
    let encrypted = sentinal::encryption::encrypt_combined(&obfuscated, &KEY).unwrap();
    let asset_path = idea_path.join("Assets").join(format!("{hash}.shuffled"));
    std::fs::write(&asset_path, &encrypted).unwrap();

    let result = archivist::read(&hash, &idea_path, &KEY, &VOCAB_SEED);
    assert!(matches!(result, Err(hall::HallError::AssetHashMismatch { .. })));
}

#[test]
fn minimal_package_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_path = dir.path().join("minimal.idea");

    let digit = Digit::new(
        "text".into(),
        Value::String("just a note".into()),
        "cpub1minimal".into(),
    )
    .unwrap();
    let root_id = digit.id();
    let header = Header {
        version: "1.0".into(),
        id: Uuid::new_v4(),
        created: Utc::now(),
        modified: Utc::now(),
        extended_type: None,
        creator: Creator {
            public_key: "cpub1minimal".into(),
            signature: "sig".into(),
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

    let package = IdeaPackage::new(pkg_path.clone(), header, digit);
    scribe::write(&package, &KEY, None).unwrap();

    let result = scholar::read(&pkg_path, &KEY, None).unwrap();
    assert!(!result.has_warnings());
    assert_eq!(result.value.digits.len(), 1);
    assert!(result.value.book.is_none());
    assert!(result.value.tree.is_none());
    assert!(result.value.cool.is_none());
    assert!(result.value.redemption.is_none());
    assert!(result.value.bonds.is_none());
    assert!(result.value.position.is_none());
}
