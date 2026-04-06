use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A proximity bond — proof that two people were physically near each other.
///
/// This is the HIGHEST-WEIGHT verification method, but NOT the only one.
/// For adults, other methods (vouch, sponsor, digital, reputation, time) also work.
/// For Kids Sphere family bonds, proximity IS required (non-negotiable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProximityBond {
    pub id: Uuid,
    pub self_pubkey: String,
    pub other_pubkey: String,
    pub method: ProximityMethod,
    pub proof: ProximityProof,
    pub created_at: DateTime<Utc>,
    pub context: Option<String>,
}

impl ProximityBond {
    pub fn new(
        self_pubkey: impl Into<String>,
        other_pubkey: impl Into<String>,
        method: ProximityMethod,
        proof: ProximityProof,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            self_pubkey: self_pubkey.into(),
            other_pubkey: other_pubkey.into(),
            method,
            proof,
            created_at: Utc::now(),
            context: None,
        }
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn involves(&self, pubkey: &str) -> bool {
        self.self_pubkey == pubkey || self.other_pubkey == pubkey
    }
}

/// How proximity was verified.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProximityMethod {
    /// QR code scan + Bluetooth ranging.
    QrBluetooth,
    /// QR code scan + ultrasonic handshake (18-22kHz).
    QrUltrasonic,
    /// NFC tap (~10cm range).
    Nfc,
    /// QR code scan + NFC confirmation.
    QrNfc,
}

/// Evidence of physical proximity — anti-circumvention proof.
///
/// Nonce has 60-second TTL. At least one proximity signal required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProximityProof {
    pub nonce: String,
    pub nonce_created_at: DateTime<Utc>,
    pub nonce_expires_at: DateTime<Utc>,
    pub ble_rssi: Option<i32>,
    pub ultrasonic_response: Option<String>,
    pub nfc_token: Option<String>,
}

impl ProximityProof {
    /// Create a new proof with a nonce (60-second TTL).
    pub fn new(nonce: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            nonce: nonce.into(),
            nonce_created_at: now,
            nonce_expires_at: now + chrono::Duration::seconds(NONCE_TTL_SECONDS),
            ble_rssi: None,
            ultrasonic_response: None,
            nfc_token: None,
        }
    }

    pub fn with_ble(mut self, rssi: i32) -> Self {
        self.ble_rssi = Some(rssi);
        self
    }

    pub fn with_ultrasonic(mut self, response: impl Into<String>) -> Self {
        self.ultrasonic_response = Some(response.into());
        self
    }

    pub fn with_nfc(mut self, token: impl Into<String>) -> Self {
        self.nfc_token = Some(token.into());
        self
    }

    /// Whether at least one proximity signal is present and valid.
    pub fn has_proximity_evidence(&self) -> bool {
        self.has_valid_ble() || self.ultrasonic_response.is_some() || self.nfc_token.is_some()
    }

    /// BLE RSSI must be >= -55 dBm (~3 meters).
    pub fn has_valid_ble(&self) -> bool {
        self.ble_rssi
            .is_some_and(|rssi| rssi >= REQUIRED_BLE_RSSI)
    }

    /// Whether the nonce has expired.
    pub fn is_nonce_expired(&self) -> bool {
        Utc::now() > self.nonce_expires_at
    }

    /// Whether this proof is valid (has evidence + nonce not expired).
    pub fn is_valid(&self) -> bool {
        self.has_proximity_evidence() && !self.is_nonce_expired()
    }
}

/// BLE RSSI threshold — -55 dBm ≈ 3 meters.
pub const REQUIRED_BLE_RSSI: i32 = -55;

/// Nonce time-to-live in seconds.
pub const NONCE_TTL_SECONDS: i64 = 60;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proximity_proof_with_ble() {
        let proof = ProximityProof::new("test_nonce")
            .with_ble(-45);
        assert!(proof.has_valid_ble());
        assert!(proof.has_proximity_evidence());
        assert!(proof.is_valid());
    }

    #[test]
    fn ble_too_weak() {
        let proof = ProximityProof::new("test_nonce")
            .with_ble(-70); // too far away
        assert!(!proof.has_valid_ble());
        assert!(!proof.has_proximity_evidence());
    }

    #[test]
    fn proximity_proof_with_nfc() {
        let proof = ProximityProof::new("test_nonce")
            .with_nfc("nfc_session_token");
        assert!(proof.has_proximity_evidence());
        assert!(proof.is_valid());
    }

    #[test]
    fn no_evidence_invalid() {
        let proof = ProximityProof::new("test_nonce");
        assert!(!proof.has_proximity_evidence());
        assert!(!proof.is_valid());
    }

    #[test]
    fn expired_nonce() {
        let mut proof = ProximityProof::new("test_nonce")
            .with_ble(-40);
        proof.nonce_expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(!proof.is_valid());
        assert!(proof.is_nonce_expired());
    }

    #[test]
    fn proximity_bond_creation() {
        let proof = ProximityProof::new("nonce123").with_ble(-50);
        let bond = ProximityBond::new("alice", "bob", ProximityMethod::QrBluetooth, proof)
            .with_context("Met at the community garden");
        assert!(bond.involves("alice"));
        assert!(bond.involves("bob"));
        assert!(!bond.involves("charlie"));
    }
}
