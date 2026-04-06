use crown::*;

#[test]
fn identity_lifecycle() {
    let mut keyring = Keyring::new();
    assert!(!keyring.is_unlocked());

    let kp = keyring.generate_primary().unwrap();
    let crown_id = kp.crown_id().to_string();
    assert!(keyring.is_unlocked());

    // Sign and verify.
    let data = b"sovereign data";
    let sig = keyring.sign(data).unwrap();
    assert!(sig.verify_signer(data));
    assert_eq!(sig.signer(), crown_id);

    // Lock clears everything.
    keyring.lock();
    assert!(!keyring.is_unlocked());
    assert!(keyring.sign(data).is_err());
}

#[test]
fn keyring_export_import_preserves_identity() {
    let mut kr1 = Keyring::new();
    kr1.generate_primary().unwrap();
    kr1.create_persona("work").unwrap();
    kr1.create_persona("anon").unwrap();

    let exported = kr1.export().unwrap();

    let mut kr2 = Keyring::new();
    kr2.load(&exported).unwrap();

    // All keys match.
    assert_eq!(kr1.public_key().unwrap(), kr2.public_key().unwrap());
    assert_eq!(
        kr1.public_key_for("work").unwrap(),
        kr2.public_key_for("work").unwrap()
    );
    assert_eq!(
        kr1.public_key_for("anon").unwrap(),
        kr2.public_key_for("anon").unwrap()
    );

    // Signing still works after import.
    let sig = kr2.sign(b"test").unwrap();
    assert!(sig.verify_signer(b"test"));
}

#[test]
fn soul_persistence_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let soul_path = dir.path().join("soul.idea");

    let mut soul = Soul::create(&soul_path, None).unwrap();
    let mut profile = soul.profile().clone();
    profile.display_name = Some("Sam".into());
    profile.bio = Some("Builder of sovereign internets".into());
    soul.update_profile(profile);
    soul.follow("cpub1friend");
    soul.save().unwrap();

    let loaded = Soul::load(&soul_path, None).unwrap();
    assert_eq!(loaded.profile().display_name.as_deref(), Some("Sam"));
    assert_eq!(
        loaded.profile().bio.as_deref(),
        Some("Builder of sovereign internets")
    );
    assert!(loaded.social_graph().is_following("cpub1friend"));
}

#[test]
fn cross_persona_signing() {
    let mut keyring = Keyring::new();
    keyring.generate_primary().unwrap();
    keyring.create_persona("alt").unwrap();

    let data = b"persona test";
    let sig_primary = keyring.sign(data).unwrap();
    let sig_alt = keyring.sign_as(data, "alt").unwrap();

    // Different signers.
    assert_ne!(sig_primary.signer(), sig_alt.signer());

    // Both verify.
    assert!(sig_primary.verify_signer(data));
    assert!(sig_alt.verify_signer(data));

    // Cross-verify fails.
    assert!(!sig_primary.verify_crown_id(data, sig_alt.signer()));
    assert!(!sig_alt.verify_crown_id(data, sig_primary.signer()));
}

#[test]
fn block_removes_from_following_integration() {
    let mut soul = Soul::new();
    soul.follow("cpub1alice");
    assert!(soul.social_graph().is_following("cpub1alice"));

    soul.block("cpub1alice");
    assert!(!soul.social_graph().is_following("cpub1alice"));
    assert!(soul.social_graph().is_blocked("cpub1alice"));
}

#[test]
fn verify_with_public_only_keypair() {
    let signer = CrownKeypair::generate();
    let crown_id_str = signer.crown_id().to_string();

    let sig = Signature::sign(b"message", &signer).unwrap();

    // Verifier only has the crown ID — no private key.
    let verifier = CrownKeypair::from_crown_id(&crown_id_str).unwrap();
    assert!(sig.verify(b"message", verifier.public_key_data()));
    assert!(!verifier.has_private_key());
}

#[test]
fn full_soul_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("complete.idea");

    let mut soul = Soul::create(&path, None).unwrap();

    // Profile — all fields.
    let mut profile = Profile::empty();
    profile.display_name = Some("Test User".into());
    profile.username = Some("testuser".into());
    profile.bio = Some("A sovereign citizen".into());
    profile.avatar = Some(AvatarReference::Url(
        "https://example.com/avatar.png".into(),
    ));
    profile.website = Some("https://example.com".into());
    profile.lightning_address = Some("test@example.com".into());
    profile.verification_address = Some("test@example.com".into());
    soul.update_profile(profile);

    // Preferences — modified.
    let prefs = Preferences {
        theme: Theme::Cosmic,
        text_scale: 1.2,
        default_visibility: Visibility::Public,
        ..Default::default()
    };
    soul.update_preferences(prefs);

    // Social — populated.
    soul.follow("cpub1alice");
    soul.follow("cpub1bob");
    soul.block("cpub1evil");

    soul.save().unwrap();

    // Reload and verify everything.
    let loaded = Soul::load(&path, None).unwrap();
    assert_eq!(loaded.profile().username.as_deref(), Some("testuser"));
    assert_eq!(loaded.profile().verification_address.as_deref(), Some("test@example.com"));
    assert_eq!(loaded.preferences().theme, Theme::Cosmic);
    assert!((loaded.preferences().text_scale - 1.2).abs() < f64::EPSILON);
    assert_eq!(loaded.preferences().default_visibility, Visibility::Public);
    assert_eq!(loaded.social_graph().following.len(), 2);
    assert!(loaded.social_graph().is_blocked("cpub1evil"));
    assert!(!loaded.social_graph().is_following("cpub1evil"));
}
