use sentinal::encryption;
use sentinal::key_derivation;
use sentinal::key_slot::{InternalKeySlot, KeySlotCredential, PasswordKeySlot, PublicKeySlot};
use sentinal::obfuscation;
use sentinal::recovery;
use sentinal::secure_data::SecureData;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

/// Full key hierarchy: password → master key → content key → encrypt/decrypt.
#[test]
fn full_key_hierarchy_round_trip() {
    let password = "sovereignty-is-a-birthright";
    let idea_id = Uuid::new_v4();

    // 1. Derive master key from password.
    let (master_key, salt) = key_derivation::derive_master_key(password, None).unwrap();
    assert_eq!(master_key.len(), 32);

    // 2. Derive content key for this idea.
    let content_key = key_derivation::derive_content_key(master_key.expose(), &idea_id).unwrap();
    assert_eq!(content_key.len(), 32);

    // 3. Encrypt content.
    let plaintext = b"Dignity cannot be taken, traded, or measured.";
    let encrypted = encryption::encrypt(plaintext, content_key.expose()).unwrap();

    // 4. Decrypt content.
    let decrypted = encryption::decrypt(&encrypted, content_key.expose()).unwrap();
    assert_eq!(decrypted, plaintext);

    // 5. Re-derive the same content key (deterministic).
    let (master_key2, _) = key_derivation::derive_master_key(password, Some(&salt)).unwrap();
    let content_key2 = key_derivation::derive_content_key(master_key2.expose(), &idea_id).unwrap();
    assert_eq!(content_key, content_key2);
}

/// Password key slot: protect a content key with a password.
#[test]
fn password_key_slot_full_flow() {
    let password = "the-covenant-governs";

    // Generate a random content key.
    let content_key = SecureData::random(32).unwrap();

    // Create a password-protected slot.
    let slot = PasswordKeySlot::create(content_key.expose(), password).unwrap();

    // Serialize to JSON and back (simulates persisting the slot).
    let json = serde_json::to_string(&slot).unwrap();
    let restored: sentinal::KeySlot = serde_json::from_str(&json).unwrap();

    // Unwrap with the correct password.
    let recovered = restored
        .unwrap(KeySlotCredential::Password(password))
        .unwrap();
    assert_eq!(recovered.expose(), content_key.expose());
}

/// Public key slot: share a content key via X25519 ECDH.
#[test]
fn public_key_slot_sharing_flow() {
    // Alice has content she wants to share with Bob.
    let content_key = SecureData::random(32).unwrap();

    // Bob's keypair.
    let bob_secret = StaticSecret::random();
    let bob_public = PublicKey::from(&bob_secret);

    // Alice creates a slot for Bob.
    let slot = PublicKeySlot::create(
        content_key.expose(),
        bob_public.as_bytes(),
        "cpub1bob",
    )
    .unwrap();

    // Serialize (would be sent over Globe in practice).
    let json = serde_json::to_string(&slot).unwrap();
    let restored: sentinal::KeySlot = serde_json::from_str(&json).unwrap();

    // Bob unwraps with his private key.
    let recovered = restored
        .unwrap(KeySlotCredential::PrivateKey(bob_secret.as_bytes()))
        .unwrap();
    assert_eq!(recovered.expose(), content_key.expose());
}

/// Internal key slot: vault dimension hierarchy.
#[test]
fn internal_key_slot_dimension_flow() {
    let master = SecureData::random(32).unwrap();
    let dimension_id = Uuid::new_v4();

    // Derive a dimension key from the master key.
    let dimension_key =
        key_derivation::derive_dimension_key(master.expose(), &dimension_id).unwrap();

    // Create a content key and protect it with the dimension key.
    let content_key = SecureData::random(32).unwrap();
    let slot = InternalKeySlot::create(
        content_key.expose(),
        dimension_key.expose(),
        dimension_id,
    )
    .unwrap();

    // Unwrap.
    let recovered = slot
        .unwrap(KeySlotCredential::DimensionKey(dimension_key.expose()))
        .unwrap();
    assert_eq!(recovered.expose(), content_key.expose());
}

/// Obfuscation pipeline: encrypt → obfuscate → deobfuscate → decrypt.
#[test]
fn obfuscation_defense_in_depth() {
    let password = "layers-of-protection";
    let (master_key, _) = key_derivation::derive_master_key(password, None).unwrap();

    // Derive content key and vocabulary seed.
    let idea_id = Uuid::new_v4();
    let content_key = key_derivation::derive_content_key(master_key.expose(), &idea_id).unwrap();
    let vocab_seed = key_derivation::derive_vocabulary_seed(master_key.expose()).unwrap();

    let plaintext = b"Consent must be voluntary, informed, continuous, and revocable.";

    // Layer 1: Obfuscate (would be Babel encode in practice).
    let obfuscated = obfuscation::obfuscate(plaintext, vocab_seed.expose());
    assert_ne!(obfuscated, plaintext);

    // Layer 2: Encrypt.
    let encrypted = encryption::encrypt_combined(&obfuscated, content_key.expose()).unwrap();

    // --- In storage / transit ---

    // Layer 2: Decrypt.
    let decrypted = encryption::decrypt_combined(&encrypted, content_key.expose()).unwrap();
    assert_eq!(decrypted, obfuscated); // Still obfuscated in memory.

    // Layer 1: Deobfuscate.
    let restored = obfuscation::deobfuscate(&decrypted, vocab_seed.expose());
    assert_eq!(restored, plaintext);
}

/// Recovery phrase: generate → validate → derive seed.
#[test]
fn recovery_phrase_full_flow() {
    let phrase = recovery::generate_phrase().unwrap();
    assert_eq!(phrase.len(), 24);
    assert!(recovery::validate_phrase(&phrase));

    let seed = recovery::phrase_to_seed(&phrase, "optional-passphrase").unwrap();
    assert_eq!(seed.len(), 64);

    // Same phrase + passphrase = same seed.
    let seed2 = recovery::phrase_to_seed(&phrase, "optional-passphrase").unwrap();
    assert_eq!(seed, seed2);
}
