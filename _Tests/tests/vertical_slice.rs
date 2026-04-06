//! Vertical Slice — Phase 3 Gate Test
//!
//! Proves: "A person has an identity and can connect to Omnidea relays."
//!
//! Alice and Bob generate Crown identities, derive a shared Babel vocabulary
//! via ECDH, and communicate through a Globe relay. Alice creates a full .idea
//! Digit, Babel-encodes the content, signs an ORP event, and publishes. Bob
//! receives, verifies, and Babel-decodes — in multiple languages.
//!
//! Crates exercised: Crown, Globe, Lingo, Ideas, Sentinal, X.

use crown::CrownKeypair;
use futures_util::{SinkExt, StreamExt};
use globe::*;
use ideas::Digit;
use lingo::Babel;
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::Message;
use x::Value;

/// Derive a shared Babel vocabulary seed from two keypairs via ECDH.
///
/// Both sides independently arrive at the same seed:
/// `shared_babel_seed(alice, bob_pub) == shared_babel_seed(bob, alice_pub)`
fn shared_babel_seed(my_keypair: &CrownKeypair, their_pubkey: &[u8; 32]) -> Vec<u8> {
    let shared_secret = my_keypair.shared_secret(their_pubkey).unwrap();
    // Domain-separated derivation: SHA-256(shared_secret || context)
    let mut hasher = Sha256::new();
    hasher.update(shared_secret);
    hasher.update(b"omnidea-babel-shared-v1");
    hasher.finalize().to_vec()
}

/// Helper: connect to relay, publish event, assert OK.
async fn publish_event(url: &str, event: &OmniEvent) {
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    let msg = ClientMessage::Event(event.clone());
    write
        .send(Message::Text(msg.to_json().unwrap().into()))
        .await
        .unwrap();

    let response = read.next().await.unwrap().unwrap();
    match RelayMessage::from_json(response.to_text().unwrap()).unwrap() {
        RelayMessage::Ok { success, .. } => assert!(success, "relay should accept event"),
        other => panic!("expected OK, got: {other:?}"),
    }
}

/// Helper: connect to relay, subscribe, receive first event.
async fn subscribe_and_receive(url: &str, sub_id: &str, filter: OmniFilter) -> OmniEvent {
    let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    let sub_msg = ClientMessage::Req {
        subscription_id: sub_id.into(),
        filters: vec![filter],
    };
    write
        .send(Message::Text(sub_msg.to_json().unwrap().into()))
        .await
        .unwrap();

    let received = read.next().await.unwrap().unwrap();
    match RelayMessage::from_json(received.to_text().unwrap()).unwrap() {
        RelayMessage::Event { event, .. } => event,
        other => panic!("expected EVENT, got: {other:?}"),
    }
}

// ==========================================================================
// Test 1: The Full Vertical Slice (English)
// ==========================================================================

#[tokio::test]
async fn vertical_slice_english() {
    // --- Identity (Crown) ---
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();

    // --- Shared Babel Vocabulary (Crown ECDH → Lingo) ---
    let seed_alice = shared_babel_seed(&alice, bob.public_key_data());
    let seed_bob = shared_babel_seed(&bob, alice.public_key_data());
    assert_eq!(seed_alice, seed_bob, "ECDH produces identical seeds");

    let babel_alice = Babel::new(&seed_alice);
    let babel_bob = Babel::new(&seed_bob);

    // --- Content (Ideas) ---
    let original_text = "Hello from the sovereign internet! Welcome to Omnidea.";
    let digit = Digit::new(
        "text.note".into(),
        Value::from(original_text),
        alice.crown_id().to_string(),
    )
    .unwrap();

    // Babel-encode the human-readable content (not the JSON structure).
    let encoded_text = babel_alice.encode(original_text);
    assert_ne!(encoded_text, original_text);

    // Build ORP event: Babel-encoded content + metadata as tags.
    let unsigned = UnsignedEvent::new(9001, &encoded_text)
        .with_tag("lang", &["en"])
        .with_tag("p", &[&bob.public_key_hex()])
        .with_tag("digit-id", &[&digit.id().to_string()])
        .with_tag("digit-type", &["text.note"])
        .with_application_tag("omnidea");

    let event = EventBuilder::sign(&unsigned, &alice).unwrap();
    assert!(event.validate().is_ok());
    assert!(EventBuilder::verify(&event).unwrap());

    // --- Start Relay (Globe) ---
    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = format!("ws://{addr}");

    // --- Alice publishes ---
    publish_event(&url, &event).await;

    // --- Bob subscribes and receives ---
    let filter = OmniFilter {
        authors: Some(vec![alice.public_key_hex()]),
        ..Default::default()
    };
    let received_event = subscribe_and_receive(&url, "bob-sub", filter).await;

    // --- Bob verifies signature (Crown) ---
    assert!(EventBuilder::verify(&received_event).unwrap());
    assert_eq!(received_event.author, alice.public_key_hex());

    // --- Bob Babel-decodes (Lingo) ---
    let decoded_text = babel_bob.decode(&received_event.content);
    assert_eq!(decoded_text, original_text);

    // --- Bob has the full .idea context ---
    assert!(received_event.has_tag("digit-type", "text.note"));
    assert!(received_event.has_tag("lang", "en"));
    assert!(received_event.has_tag("application", "omnidea"));
}

// ==========================================================================
// Test 2: Omnilingual — Kenji (Japanese) → Alice (English reader)
// ==========================================================================

#[tokio::test]
async fn vertical_slice_omnilingual() {
    let alice = CrownKeypair::generate();
    let kenji = CrownKeypair::generate();

    // Shared vocabulary via ECDH.
    let seed = shared_babel_seed(&kenji, alice.public_key_data());
    let babel_kenji = Babel::new(&seed);
    let babel_alice = Babel::new(&shared_babel_seed(&alice, kenji.public_key_data()));

    // Kenji writes in Japanese (common tokens for round-trip).
    let kenji_text = "の に は を た";
    let digit = Digit::new(
        "text.note".into(),
        Value::from(kenji_text),
        kenji.crown_id().to_string(),
    )
    .unwrap();

    let encoded = babel_kenji.encode(kenji_text);
    assert_ne!(encoded, kenji_text);

    // Sign and publish.
    let unsigned = UnsignedEvent::new(9001, &encoded)
        .with_tag("lang", &["ja"])
        .with_tag("digit-id", &[&digit.id().to_string()])
        .with_application_tag("omnidea");
    let event = EventBuilder::sign(&unsigned, &kenji).unwrap();

    let (_server, addr) = RelayServer::start_on_available_port().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let url = format!("ws://{addr}");

    publish_event(&url, &event).await;

    // Alice receives.
    let filter = OmniFilter {
        authors: Some(vec![kenji.public_key_hex()]),
        ..Default::default()
    };
    let received = subscribe_and_receive(&url, "alice-sub", filter).await;

    // Alice verifies Kenji's signature.
    assert!(EventBuilder::verify(&received).unwrap());

    // Alice Babel-decodes with language-aware rejoin (no spaces for Japanese).
    let decoded = babel_alice.decode_for_language(&received.content, "ja");
    assert_eq!(decoded, "のにはをた");

    // Without language awareness, spaces would be wrong.
    let naive_decode = babel_alice.decode(&received.content);
    assert!(naive_decode.contains(' '), "naive decode has spaces");
    assert!(!decoded.contains(' '), "language-aware decode has no spaces");
}

// ==========================================================================
// Test 3: ECDH Vocabulary Isolation — Every pair has its own language
// ==========================================================================

#[test]
fn ecdh_vocabulary_isolation() {
    let alice = CrownKeypair::generate();
    let bob = CrownKeypair::generate();
    let gertrude = CrownKeypair::generate();

    // Alice ↔ Bob: one vocabulary.
    let babel_ab = Babel::new(&shared_babel_seed(&alice, bob.public_key_data()));

    // Alice ↔ Gertrude: different vocabulary.
    let babel_ag = Babel::new(&shared_babel_seed(&alice, gertrude.public_key_data()));

    let text = "hello world omnidea";
    let encoded_ab = babel_ab.encode(text);
    let encoded_ag = babel_ag.encode(text);

    // Different pairs → different encodings.
    assert_ne!(encoded_ab, encoded_ag);

    // Each pair decodes its own.
    let babel_ba = Babel::new(&shared_babel_seed(&bob, alice.public_key_data()));
    assert_eq!(babel_ba.decode(&encoded_ab), text);

    let babel_ga = Babel::new(&shared_babel_seed(&gertrude, alice.public_key_data()));
    assert_eq!(babel_ga.decode(&encoded_ag), text);

    // Cross-pair decoding fails (wrong vocabulary).
    assert_ne!(
        babel_ga.decode(&encoded_ab),
        text,
        "Gertrude should NOT decode Alice↔Bob content"
    );
}

// ==========================================================================
// Test 4: Personal Babel — Vault-level memory protection
// ==========================================================================

#[test]
fn personal_babel_vault_protection() {
    let alice = CrownKeypair::generate();

    // Alice's personal vocabulary from her master key.
    let master_key = vec![0x42u8; 32];
    let vocab_seed = sentinal::key_derivation::derive_vocabulary_seed(&master_key).unwrap();
    let babel = Babel::new(vocab_seed.expose());

    // The content to protect.
    let secret_text = "My private thoughts about the sovereign internet.";

    let digit = Digit::new(
        "journal.entry".into(),
        Value::from(secret_text),
        alice.crown_id().to_string(),
    )
    .unwrap();

    // Babel-encode the content for vault storage.
    let encoded = babel.encode(secret_text);
    assert_ne!(encoded, secret_text);
    assert!(!encoded.contains("private"));
    assert!(!encoded.contains("sovereign"));

    // Decode with same seed → recovered.
    let decoded = babel.decode(&encoded);
    assert_eq!(decoded, secret_text);

    // Wrong master key → wrong vocabulary → cannot decode.
    let wrong_seed = sentinal::key_derivation::derive_vocabulary_seed(&[0x99u8; 32]).unwrap();
    let wrong_babel = Babel::new(wrong_seed.expose());
    assert_ne!(wrong_babel.decode(&encoded), secret_text);

    // The Digit itself is valid and holds the original content.
    assert_eq!(digit.author(), alice.crown_id());
    assert_eq!(digit.digit_type(), "journal.entry");
    assert_eq!(digit.content, Value::from(secret_text));
}

// ==========================================================================
// Test 5: Full .idea Digit — properties, children, Babel round-trip
// ==========================================================================

#[test]
fn full_idea_digit_structure() {
    let alice = CrownKeypair::generate();

    // Create a rich Digit.
    let mut digit = Digit::new(
        "document.page".into(),
        Value::from("Page 1 content"),
        alice.crown_id().to_string(),
    )
    .unwrap();
    digit.properties.insert(
        "title".into(),
        Value::from("My First Omnidea Document"),
    );
    let child_id = uuid::Uuid::new_v4();
    digit.children = Some(vec![child_id]);

    // JSON round-trip.
    let json = serde_json::to_string(&digit).unwrap();
    let recovered: Digit = serde_json::from_str(&json).unwrap();
    assert_eq!(recovered.id(), digit.id());
    assert_eq!(recovered.digit_type(), "document.page");
    assert_eq!(recovered.author(), alice.crown_id());
    assert_eq!(
        recovered.properties.get("title").unwrap(),
        &Value::from("My First Omnidea Document")
    );
    assert_eq!(recovered.children.as_ref().unwrap()[0], child_id);

    // Babel round-trips the ENTIRE serialized Digit JSON — including UUIDs,
    // timestamps, special characters, everything. No more lossy encoding.
    let babel = Babel::new(b"test-seed-for-digit-babel-00032");
    let encoded_json = babel.encode(&json);
    assert_ne!(encoded_json, json);
    let decoded_json = babel.decode(&encoded_json);
    assert_eq!(decoded_json, json, "full Digit JSON should round-trip through Babel");

    // Verify the decoded JSON is still valid and deserializes correctly.
    let round_tripped: Digit = serde_json::from_str(&decoded_json).unwrap();
    assert_eq!(round_tripped.id(), digit.id());
    assert_eq!(round_tripped.author(), alice.crown_id());
}
