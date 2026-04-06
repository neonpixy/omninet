//! Pairing protocol — wraps Globe's pairing data types with real
//! cryptographic verification using Crown's BIP-340 Schnorr signatures.
//!
//! # Flow
//!
//! 1. **Initiator** calls [`PairingProtocol::initiate`] to create a
//!    [`PairingChallenge`] (displayed as a QR code or sent via mDNS).
//! 2. **Responder** calls [`PairingProtocol::respond`] to sign the
//!    challenge nonce with their Crown key.
//! 3. **Initiator** calls [`PairingProtocol::verify`] to validate the
//!    signature and produce a [`DevicePair`].
//!
//! The challenge expires after 5 minutes to prevent replay attacks.

use chrono::Utc;
use crown::{CrownKeypair, Signature};
use globe::discovery::pairing::{DevicePair, PairingChallenge, PairingResponse};
use rand::Rng;

use crate::error::DeviceManagerError;

/// Challenge expiry duration in seconds (5 minutes).
const CHALLENGE_EXPIRY_SECS: i64 = 300;

/// Stateless pairing protocol operations.
///
/// Each method is a pure function over Globe's pairing types and
/// Crown's cryptographic primitives. No state is held between calls.
pub struct PairingProtocol;

impl PairingProtocol {
    /// Create a pairing challenge.
    ///
    /// The initiator generates a random 32-byte nonce and sets the
    /// challenge to expire in 5 minutes. The resulting challenge is
    /// typically displayed as a QR code or broadcast via mDNS.
    ///
    /// # Arguments
    ///
    /// * `keypair` - The initiator's Crown identity keypair.
    /// * `device_name` - Human-readable device name (e.g. "Sam's MacBook Pro").
    /// * `relay_url` - Relay URL where the pairing response should be sent.
    pub fn initiate(
        keypair: &CrownKeypair,
        device_name: &str,
        relay_url: &str,
    ) -> PairingChallenge {
        let mut nonce_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = hex::encode(nonce_bytes);

        let expires_at = Utc::now().timestamp() + CHALLENGE_EXPIRY_SECS;

        PairingChallenge {
            nonce,
            device_crown_id: keypair.public_key_hex(),
            relay_url: relay_url.to_string(),
            expires_at,
            device_name: device_name.to_string(),
        }
    }

    /// Respond to a pairing challenge by signing the nonce.
    ///
    /// The responder signs `challenge.nonce.as_bytes()` with their
    /// Crown key. The signature is hex-encoded into the response.
    ///
    /// # Errors
    ///
    /// Returns [`DeviceManagerError::PairingExpired`] if the challenge
    /// has passed its `expires_at` timestamp.
    ///
    /// Returns [`DeviceManagerError::PairingFailed`] if signing fails
    /// (e.g. the keypair has no private key).
    pub fn respond(
        challenge: &PairingChallenge,
        keypair: &CrownKeypair,
        device_name: &str,
    ) -> Result<PairingResponse, DeviceManagerError> {
        // Check expiry.
        let now = Utc::now().timestamp();
        if now > challenge.expires_at {
            return Err(DeviceManagerError::PairingExpired);
        }

        // Sign the nonce bytes with Crown's BIP-340 Schnorr.
        let sig = Signature::sign(challenge.nonce.as_bytes(), keypair)
            .map_err(|e| DeviceManagerError::PairingFailed(e.to_string()))?;

        Ok(PairingResponse {
            nonce: challenge.nonce.clone(),
            responder_crown_id: keypair.crown_id().to_string(),
            signature: sig.hex(),
            device_name: device_name.to_string(),
        })
    }

    /// Verify a pairing response and produce a [`DevicePair`].
    ///
    /// Checks that:
    /// 1. The response nonce matches the challenge nonce.
    /// 2. The challenge has not expired.
    /// 3. The BIP-340 Schnorr signature is valid for the nonce bytes
    ///    and the responder's public key.
    ///
    /// # Errors
    ///
    /// - [`DeviceManagerError::NonceMismatch`] — nonces don't match.
    /// - [`DeviceManagerError::PairingExpired`] — challenge has expired.
    /// - [`DeviceManagerError::SignatureInvalid`] — bad or tampered signature.
    /// - [`DeviceManagerError::PairingFailed`] — malformed signature bytes.
    pub fn verify(
        challenge: &PairingChallenge,
        response: &PairingResponse,
    ) -> Result<DevicePair, DeviceManagerError> {
        // 1. Nonce must match.
        if challenge.nonce != response.nonce {
            return Err(DeviceManagerError::NonceMismatch);
        }

        // 2. Challenge must not be expired.
        let now = Utc::now().timestamp();
        if now > challenge.expires_at {
            return Err(DeviceManagerError::PairingExpired);
        }

        // 3. Decode the hex signature (128 hex chars = 64 bytes).
        let sig_bytes: [u8; 64] = hex::decode(&response.signature)
            .map_err(|e| DeviceManagerError::PairingFailed(format!("invalid hex signature: {e}")))?
            .try_into()
            .map_err(|v: Vec<u8>| {
                DeviceManagerError::PairingFailed(format!(
                    "expected 64-byte signature, got {} bytes",
                    v.len()
                ))
            })?;

        // 4. Reconstruct the Crown Signature and verify against the responder's crown_id.
        let crown_sig = Signature::new(sig_bytes, response.responder_crown_id.clone(), Utc::now());

        if !crown_sig.verify_crown_id(challenge.nonce.as_bytes(), &response.responder_crown_id) {
            return Err(DeviceManagerError::SignatureInvalid);
        }

        // 5. Build the DevicePair.
        Ok(DevicePair {
            identity: response.responder_crown_id.clone(),
            local_device: challenge.device_name.clone(),
            remote_device: response.device_name.clone(),
            remote_relay_url: challenge.relay_url.clone(),
            paired_at: now,
            active: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initiate_produces_valid_challenge() {
        let kp = CrownKeypair::generate();
        let challenge = PairingProtocol::initiate(&kp, "Test Device", "ws://localhost:8080");

        // Nonce is 32 bytes hex-encoded = 64 hex chars.
        assert_eq!(challenge.nonce.len(), 64);
        assert!(challenge
            .nonce
            .chars()
            .all(|c| c.is_ascii_hexdigit()));

        // Device crown_id is the public key hex.
        assert_eq!(challenge.device_crown_id, kp.public_key_hex());

        // Relay URL is preserved.
        assert_eq!(challenge.relay_url, "ws://localhost:8080");

        // Device name is preserved.
        assert_eq!(challenge.device_name, "Test Device");

        // Expiry is in the future (within 5 minutes + small tolerance).
        let now = Utc::now().timestamp();
        assert!(challenge.expires_at > now);
        assert!(challenge.expires_at <= now + CHALLENGE_EXPIRY_SECS + 1);
    }

    #[test]
    fn respond_signs_nonce_correctly() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        let challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        let response =
            PairingProtocol::respond(&challenge, &responder, "Phone").expect("respond should succeed");

        // Nonce is echoed.
        assert_eq!(response.nonce, challenge.nonce);

        // Responder crown_id is correct.
        assert_eq!(response.responder_crown_id, responder.crown_id());

        // Signature is valid hex (128 chars = 64 bytes).
        assert_eq!(response.signature.len(), 128);
        assert!(response.signature.chars().all(|c| c.is_ascii_hexdigit()));

        // Device name is preserved.
        assert_eq!(response.device_name, "Phone");
    }

    #[test]
    fn verify_accepts_valid_response() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        let challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        let response =
            PairingProtocol::respond(&challenge, &responder, "Phone").expect("respond should succeed");

        let pair =
            PairingProtocol::verify(&challenge, &response).expect("verify should succeed");

        assert_eq!(pair.identity, responder.crown_id());
        assert_eq!(pair.local_device, "Desktop");
        assert_eq!(pair.remote_device, "Phone");
        assert_eq!(pair.remote_relay_url, "ws://localhost:8080");
        assert!(pair.active);
    }

    #[test]
    fn verify_rejects_expired_challenge() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        // Create a challenge that already expired.
        let mut challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        challenge.expires_at = Utc::now().timestamp() - 1;

        let result = PairingProtocol::respond(&challenge, &responder, "Phone");
        assert!(matches!(result, Err(DeviceManagerError::PairingExpired)));
    }

    #[test]
    fn verify_rejects_nonce_mismatch() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        let challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        let mut response =
            PairingProtocol::respond(&challenge, &responder, "Phone").expect("respond should succeed");

        // Tamper with the nonce in the response.
        response.nonce = "aaaa".repeat(16);

        let result = PairingProtocol::verify(&challenge, &response);
        assert!(matches!(result, Err(DeviceManagerError::NonceMismatch)));
    }

    #[test]
    fn verify_rejects_invalid_signature() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        let challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        let mut response =
            PairingProtocol::respond(&challenge, &responder, "Phone").expect("respond should succeed");

        // Tamper with the signature (flip a byte).
        let mut sig_bytes = hex::decode(&response.signature).unwrap();
        sig_bytes[0] ^= 0xff;
        response.signature = hex::encode(&sig_bytes);

        let result = PairingProtocol::verify(&challenge, &response);
        assert!(matches!(result, Err(DeviceManagerError::SignatureInvalid)));
    }

    #[test]
    fn verify_rejects_wrong_key_signature() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();
        let impostor = CrownKeypair::generate();

        let challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");

        // Sign with the impostor's key but claim to be the responder.
        let mut response =
            PairingProtocol::respond(&challenge, &impostor, "Phone").expect("respond should succeed");
        response.responder_crown_id = responder.crown_id().to_string();

        let result = PairingProtocol::verify(&challenge, &response);
        assert!(matches!(result, Err(DeviceManagerError::SignatureInvalid)));
    }

    #[test]
    fn verify_rejects_expired_during_verification() {
        let initiator = CrownKeypair::generate();
        let responder = CrownKeypair::generate();

        let mut challenge = PairingProtocol::initiate(&initiator, "Desktop", "ws://localhost:8080");
        let response =
            PairingProtocol::respond(&challenge, &responder, "Phone").expect("respond should succeed");

        // Expire the challenge after the response was created but before verification.
        challenge.expires_at = Utc::now().timestamp() - 1;

        let result = PairingProtocol::verify(&challenge, &response);
        assert!(matches!(result, Err(DeviceManagerError::PairingExpired)));
    }

    #[test]
    fn full_round_trip() {
        let initiator_kp = CrownKeypair::generate();
        let responder_kp = CrownKeypair::generate();

        // Step 1: Initiator creates challenge.
        let challenge =
            PairingProtocol::initiate(&initiator_kp, "MacBook Pro", "ws://192.168.1.42:8080");

        // Step 2: Responder signs it.
        let response = PairingProtocol::respond(&challenge, &responder_kp, "iPhone 16")
            .expect("respond should succeed");

        // Step 3: Initiator verifies.
        let pair =
            PairingProtocol::verify(&challenge, &response).expect("verify should succeed");

        // Assertions on the resulting pair.
        assert_eq!(pair.identity, responder_kp.crown_id());
        assert_eq!(pair.local_device, "MacBook Pro");
        assert_eq!(pair.remote_device, "iPhone 16");
        assert_eq!(pair.remote_relay_url, "ws://192.168.1.42:8080");
        assert!(pair.active);
        assert!(pair.paired_at > 0);
    }
}
