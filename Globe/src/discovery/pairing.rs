use serde::{Deserialize, Serialize};

/// A device pairing challenge.
///
/// When a phone wants to pair with a desktop Omny, the desktop generates
/// a challenge (displayed as a QR code or transmitted via mDNS). The phone
/// responds by signing the challenge with its Crown key, proving it's the
/// same person.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairingChallenge {
    /// Random challenge nonce (hex).
    pub nonce: String,
    /// The device's Crown ID (hex).
    pub device_crown_id: String,
    /// Relay URL where the pairing response should be sent.
    pub relay_url: String,
    /// Unix timestamp when the challenge expires.
    pub expires_at: i64,
    /// Device name for display (e.g. "Sam's MacBook Pro").
    pub device_name: String,
}

/// A response to a pairing challenge.
///
/// The responding device signs the nonce with its Crown key. If both
/// devices share the same Crown identity (same crown_id), pairing succeeds.
/// If different identities, the responder must be in the challenger's
/// trust network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairingResponse {
    /// The original nonce being answered.
    pub nonce: String,
    /// The responding device's Crown ID (hex).
    pub responder_crown_id: String,
    /// Schnorr signature of the nonce by the responder's Crown key.
    pub signature: String,
    /// Responding device name for display.
    pub device_name: String,
}

/// A verified device pair.
///
/// After successful pairing, both devices know each other's addresses
/// and can sync directly. Stored in Vault for persistence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DevicePair {
    /// Crown ID of the paired identity.
    pub identity: String,
    /// This device's name.
    pub local_device: String,
    /// The paired device's name.
    pub remote_device: String,
    /// The paired device's relay URL for direct connection.
    pub remote_relay_url: String,
    /// When the pairing was established.
    pub paired_at: i64,
    /// Whether this pair is currently active.
    pub active: bool,
}

/// Pairing status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PairingStatus {
    /// Challenge sent, waiting for response.
    Challenging,
    /// Response received, verifying signature.
    Verifying,
    /// Pairing succeeded.
    Paired,
    /// Pairing failed (bad signature, expired, etc).
    Failed,
    /// Challenge expired without response.
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_serde() {
        let challenge = PairingChallenge {
            nonce: "deadbeef01234567".into(),
            device_crown_id: "cpub_desktop".into(),
            relay_url: "ws://192.168.1.42:8080".into(),
            expires_at: 1709654400,
            device_name: "Sam's MacBook".into(),
        };
        let json = serde_json::to_string(&challenge).unwrap();
        let loaded: PairingChallenge = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.nonce, "deadbeef01234567");
        assert_eq!(loaded.device_name, "Sam's MacBook");
    }

    #[test]
    fn response_serde() {
        let response = PairingResponse {
            nonce: "deadbeef01234567".into(),
            responder_crown_id: "cpub_phone".into(),
            signature: "schnorr_sig_hex".into(),
            device_name: "Sam's iPhone".into(),
        };
        let json = serde_json::to_string(&response).unwrap();
        let loaded: PairingResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.nonce, "deadbeef01234567");
        assert_eq!(loaded.responder_crown_id, "cpub_phone");
    }

    #[test]
    fn device_pair_serde() {
        let pair = DevicePair {
            identity: "cpub_sam".into(),
            local_device: "MacBook Pro".into(),
            remote_device: "iPhone 16".into(),
            remote_relay_url: "ws://192.168.1.42:8080".into(),
            paired_at: 1709654400,
            active: true,
        };
        let json = serde_json::to_string(&pair).unwrap();
        let loaded: DevicePair = serde_json::from_str(&json).unwrap();
        assert!(loaded.active);
        assert_eq!(loaded.remote_device, "iPhone 16");
    }

    #[test]
    fn pairing_status_serde() {
        for status in [
            PairingStatus::Challenging,
            PairingStatus::Verifying,
            PairingStatus::Paired,
            PairingStatus::Failed,
            PairingStatus::Expired,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let loaded: PairingStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded, status);
        }
    }

    #[test]
    fn pairing_status_camel_case() {
        assert_eq!(
            serde_json::to_string(&PairingStatus::Challenging).unwrap(),
            "\"challenging\""
        );
    }
}
