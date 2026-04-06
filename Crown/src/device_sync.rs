//! Device credential sync — securely transfer a Keyring to another device.
//!
//! The protocol is offer/accept/payload:
//! 1. Device A creates a SyncOffer (nonce + expiry)
//! 2. Device B responds with SyncAccept (signed nonce)
//! 3. Device A verifies the accept, then sends a SyncPayload (encrypted keyring)
//! 4. Device B decrypts the payload and loads the keyring
//!
//! Encryption uses ECDH (via Crown's `shared_secret()`) + a caller-provided
//! RecoveryEncryptor for the symmetric step.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CrownError;
use crate::keyring::Keyring;
use crate::recovery::RecoveryEncryptor;
use crate::signature::Signature;

/// Intent to sync credentials to another device.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncOffer {
    /// Human-readable name of the offering device.
    pub from_device: String,
    /// The offering device's crown ID.
    pub from_crown_id: String,
    /// Random challenge nonce (hex-encoded).
    pub nonce: String,
    /// When this offer expires (default: 5 minutes from creation).
    pub expires_at: DateTime<Utc>,
}

/// Response accepting a sync offer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncAccept {
    /// Echoed nonce from the offer.
    pub nonce: String,
    /// Responder's crown ID.
    pub responder_crown_id: String,
    /// BIP-340 Schnorr signature of the nonce by the responder.
    pub signature: Vec<u8>,
    /// Human-readable name of the accepting device.
    pub device_name: String,
}

/// Encrypted keyring payload for transfer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncPayload {
    /// The encrypted keyring data.
    pub encrypted_keyring: Vec<u8>,
    /// Sender's crown ID.
    pub from_crown_id: String,
    /// Recipient's crown ID.
    pub to_crown_id: String,
    /// When the payload was created.
    pub timestamp: DateTime<Utc>,
}

/// Status of a device sync operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    /// Offer has been sent, waiting for accept.
    OfferSent,
    /// Accept received and verified.
    AcceptReceived,
    /// Encrypted payload sent to recipient.
    PayloadSent,
    /// Sync completed successfully.
    Complete,
    /// Sync failed with reason.
    Failed(String),
    /// Offer expired before accept was received.
    Expired,
}

/// Create a sync offer from this device.
///
/// Generates a random nonce and sets a 5-minute expiry window.
pub fn create_sync_offer(
    device_name: &str,
    keyring: &Keyring,
) -> Result<SyncOffer, CrownError> {
    let crown_id = keyring.public_key()?.to_string();

    // Generate 32 random bytes as hex nonce
    let mut nonce_bytes = [0u8; 32];
    getrandom(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);

    let expires_at = Utc::now() + Duration::minutes(5);

    Ok(SyncOffer {
        from_device: device_name.to_string(),
        from_crown_id: crown_id,
        nonce,
        expires_at,
    })
}

/// Verify a sync accept response against the original offer.
///
/// Checks that:
/// 1. The nonce matches
/// 2. The offer hasn't expired
/// 3. The signature is valid (responder signed the nonce)
pub fn verify_sync_accept(
    offer: &SyncOffer,
    accept: &SyncAccept,
) -> Result<bool, CrownError> {
    // Check nonce match
    if offer.nonce != accept.nonce {
        return Err(CrownError::InvalidSyncResponse(
            "nonce mismatch".into(),
        ));
    }

    // Check expiry
    if Utc::now() > offer.expires_at {
        return Err(CrownError::SyncExpired);
    }

    // Verify signature: responder signed the nonce bytes
    let sig_bytes: [u8; 64] = accept
        .signature
        .clone()
        .try_into()
        .map_err(|v: Vec<u8>| {
            CrownError::InvalidSyncResponse(format!(
                "signature wrong length: expected 64, got {}",
                v.len()
            ))
        })?;

    let sig = Signature::new(sig_bytes, accept.responder_crown_id.clone(), Utc::now());
    Ok(sig.verify_crown_id(accept.nonce.as_bytes(), &accept.responder_crown_id))
}

/// Prepare an encrypted sync payload for the recipient.
///
/// Uses ECDH (via Crown's `shared_secret()`) to derive a shared key,
/// then encrypts the keyring export via the caller-provided encryptor.
pub fn prepare_sync_payload(
    keyring: &Keyring,
    recipient_pubkey_hex: &str,
    encryptor: &dyn RecoveryEncryptor,
) -> Result<SyncPayload, CrownError> {
    let from_crown_id = keyring.public_key()?.to_string();

    // Derive shared secret via ECDH
    let recipient_bytes = hex::decode(recipient_pubkey_hex).map_err(|e| {
        CrownError::SyncFailed(format!("invalid recipient pubkey hex: {e}"))
    })?;
    let recipient_key: [u8; 32] = recipient_bytes.try_into().map_err(|v: Vec<u8>| {
        CrownError::SyncFailed(format!(
            "recipient pubkey wrong length: expected 32, got {}",
            v.len()
        ))
    })?;

    let kp = keyring
        .primary_keypair()
        .ok_or(CrownError::NoPrimaryKey)?;
    let shared_secret = kp.shared_secret(&recipient_key)?;

    // Derive encryption key from shared secret
    let encryption_key = encryptor.derive_key_from_seed(&shared_secret)?;

    // Export and encrypt keyring
    let keyring_data = keyring.export()?;
    let encrypted = encryptor.encrypt(&keyring_data, &encryption_key)?;

    // Derive recipient crown ID from hex
    let to_crown_id = crate::keypair::CrownKeypair::encode_public(&recipient_key);

    Ok(SyncPayload {
        encrypted_keyring: encrypted,
        from_crown_id,
        to_crown_id,
        timestamp: Utc::now(),
    })
}

/// Receive and decrypt a sync payload.
///
/// Derives the shared secret with the sender, decrypts, and loads a new Keyring.
pub fn receive_sync_payload(
    payload: &SyncPayload,
    local_keyring: &Keyring,
    encryptor: &dyn RecoveryEncryptor,
) -> Result<Keyring, CrownError> {
    // Parse sender's public key from crown ID
    let sender_pubkey = crate::keypair::CrownKeypair::decode_bech32(
        &payload.from_crown_id,
        "cpub",
    )?;

    // Derive shared secret with sender
    let local_kp = local_keyring
        .primary_keypair()
        .ok_or(CrownError::NoPrimaryKey)?;
    let shared_secret = local_kp.shared_secret(&sender_pubkey)?;

    // Derive decryption key
    let decryption_key = encryptor.derive_key_from_seed(&shared_secret)?;

    // Decrypt and load
    let plaintext = encryptor.decrypt(&payload.encrypted_keyring, &decryption_key)?;
    let mut keyring = Keyring::new();
    keyring.load(&plaintext)?;
    Ok(keyring)
}

/// Generate random bytes using the rand crate.
fn getrandom(buf: &mut [u8]) {
    use rand::RngCore;
    rand::thread_rng().fill_bytes(buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recovery::RecoveryEncryptor;

    /// Simple XOR-based mock encryptor for testing.
    struct MockEncryptor;

    impl RecoveryEncryptor for MockEncryptor {
        fn encrypt(&self, plaintext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Ok(xor_bytes(plaintext, key))
        }

        fn decrypt(&self, ciphertext: &[u8], key: &[u8]) -> Result<Vec<u8>, CrownError> {
            Ok(xor_bytes(ciphertext, key))
        }

        fn derive_key_from_password(&self, password: &str) -> Result<Vec<u8>, CrownError> {
            use sha2::{Digest, Sha256};
            Ok(Sha256::digest(password.as_bytes()).to_vec())
        }

        fn derive_key_from_seed(&self, seed: &[u8]) -> Result<Vec<u8>, CrownError> {
            use sha2::{Digest, Sha256};
            Ok(Sha256::digest(seed).to_vec())
        }
    }

    fn xor_bytes(data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect()
    }

    #[test]
    fn create_sync_offer_works() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        let offer = create_sync_offer("MacBook", &kr).unwrap();

        assert_eq!(offer.from_device, "MacBook");
        assert_eq!(offer.from_crown_id, kr.public_key().unwrap());
        assert_eq!(offer.nonce.len(), 64); // 32 bytes hex-encoded
        assert!(offer.expires_at > Utc::now());
    }

    #[test]
    fn create_sync_offer_no_primary_fails() {
        let kr = Keyring::new();
        let result = create_sync_offer("Phone", &kr);
        assert!(result.is_err());
    }

    #[test]
    fn verify_sync_accept_valid() {
        let mut sender_kr = Keyring::new();
        sender_kr.generate_primary().unwrap();

        let offer = create_sync_offer("MacBook", &sender_kr).unwrap();

        // Recipient signs the nonce
        let mut recipient_kr = Keyring::new();
        recipient_kr.generate_primary().unwrap();

        let sig = recipient_kr.sign(offer.nonce.as_bytes()).unwrap();

        let accept = SyncAccept {
            nonce: offer.nonce.clone(),
            responder_crown_id: recipient_kr.public_key().unwrap().to_string(),
            signature: sig.data().to_vec(),
            device_name: "iPhone".to_string(),
        };

        assert!(verify_sync_accept(&offer, &accept).unwrap());
    }

    #[test]
    fn verify_sync_accept_wrong_nonce() {
        let mut sender_kr = Keyring::new();
        sender_kr.generate_primary().unwrap();

        let offer = create_sync_offer("MacBook", &sender_kr).unwrap();

        let mut recipient_kr = Keyring::new();
        recipient_kr.generate_primary().unwrap();

        let sig = recipient_kr.sign(b"wrong_nonce").unwrap();

        let accept = SyncAccept {
            nonce: "wrong_nonce_hex".to_string(),
            responder_crown_id: recipient_kr.public_key().unwrap().to_string(),
            signature: sig.data().to_vec(),
            device_name: "iPhone".to_string(),
        };

        let result = verify_sync_accept(&offer, &accept);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::InvalidSyncResponse(_)
        ));
    }

    #[test]
    fn verify_sync_accept_expired() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();

        // Create an already-expired offer
        let offer = SyncOffer {
            from_device: "MacBook".to_string(),
            from_crown_id: kr.public_key().unwrap().to_string(),
            nonce: "deadbeef".to_string(),
            expires_at: Utc::now() - Duration::minutes(1),
        };

        let sig = kr.sign(b"deadbeef").unwrap();

        let accept = SyncAccept {
            nonce: "deadbeef".to_string(),
            responder_crown_id: kr.public_key().unwrap().to_string(),
            signature: sig.data().to_vec(),
            device_name: "iPhone".to_string(),
        };

        let result = verify_sync_accept(&offer, &accept);
        assert!(matches!(result.unwrap_err(), CrownError::SyncExpired));
    }

    #[test]
    fn verify_sync_accept_bad_signature() {
        let mut sender_kr = Keyring::new();
        sender_kr.generate_primary().unwrap();

        let offer = create_sync_offer("MacBook", &sender_kr).unwrap();

        let mut recipient_kr = Keyring::new();
        recipient_kr.generate_primary().unwrap();

        // Sign different data than the nonce
        let sig = recipient_kr.sign(b"different data").unwrap();

        let accept = SyncAccept {
            nonce: offer.nonce.clone(),
            responder_crown_id: recipient_kr.public_key().unwrap().to_string(),
            signature: sig.data().to_vec(),
            device_name: "iPhone".to_string(),
        };

        // Signature won't match the nonce
        assert!(!verify_sync_accept(&offer, &accept).unwrap());
    }

    #[test]
    fn sync_payload_round_trip() {
        let encryptor = MockEncryptor;

        // Device A (sender)
        let mut sender_kr = Keyring::new();
        sender_kr.generate_primary().unwrap();
        sender_kr.create_persona("work").unwrap();
        let sender_cpub = sender_kr.public_key().unwrap().to_string();

        // Device B (recipient) — has its own temp key for ECDH
        let mut recipient_kr = Keyring::new();
        recipient_kr.generate_primary().unwrap();
        let recipient_hex = recipient_kr.public_key_hex().unwrap();

        // Sender prepares payload
        let payload =
            prepare_sync_payload(&sender_kr, &recipient_hex, &encryptor).unwrap();

        assert_eq!(payload.from_crown_id, sender_cpub);
        assert!(!payload.encrypted_keyring.is_empty());

        // Recipient decrypts payload
        let received =
            receive_sync_payload(&payload, &recipient_kr, &encryptor).unwrap();

        // The received keyring should have the sender's primary key
        assert_eq!(
            received.public_key().unwrap(),
            sender_kr.public_key().unwrap()
        );
    }

    #[test]
    fn sync_payload_no_primary_fails() {
        let kr = Keyring::new();
        let encryptor = MockEncryptor;
        let result = prepare_sync_payload(&kr, &"aa".repeat(32), &encryptor);
        assert!(result.is_err());
    }

    #[test]
    fn sync_payload_invalid_recipient_hex() {
        let mut kr = Keyring::new();
        kr.generate_primary().unwrap();
        let encryptor = MockEncryptor;

        let result = prepare_sync_payload(&kr, "not_hex", &encryptor);
        assert!(matches!(
            result.unwrap_err(),
            CrownError::SyncFailed(_)
        ));
    }

    #[test]
    fn sync_status_serde() {
        let statuses = vec![
            SyncStatus::OfferSent,
            SyncStatus::AcceptReceived,
            SyncStatus::PayloadSent,
            SyncStatus::Complete,
            SyncStatus::Failed("test".into()),
            SyncStatus::Expired,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let loaded: SyncStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded, status);
        }
    }

    #[test]
    fn sync_offer_serde() {
        let offer = SyncOffer {
            from_device: "MacBook".to_string(),
            from_crown_id: "cpub1test".to_string(),
            nonce: "abc123".to_string(),
            expires_at: Utc::now(),
        };

        let json = serde_json::to_string(&offer).unwrap();
        let loaded: SyncOffer = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.from_device, "MacBook");
        assert_eq!(loaded.nonce, "abc123");
    }

    #[test]
    fn sync_accept_serde() {
        let accept = SyncAccept {
            nonce: "abc123".to_string(),
            responder_crown_id: "cpub1resp".to_string(),
            signature: vec![1, 2, 3],
            device_name: "iPhone".to_string(),
        };

        let json = serde_json::to_string(&accept).unwrap();
        let loaded: SyncAccept = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.device_name, "iPhone");
    }

    #[test]
    fn sync_payload_serde() {
        let payload = SyncPayload {
            encrypted_keyring: vec![0xDE, 0xAD],
            from_crown_id: "cpub1from".to_string(),
            to_crown_id: "cpub1to".to_string(),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        let loaded: SyncPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.from_crown_id, "cpub1from");
        assert_eq!(loaded.to_crown_id, "cpub1to");
    }
}
