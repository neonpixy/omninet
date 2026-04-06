//! Re-verification attestations — third-party identity confirmation.
//!
//! An attestation is one person confirming that they verify another's
//! identity during a re-verification session.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An attestation from one person confirming another's identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReVerificationAttestation {
    /// Pubkey of the person attesting.
    pub attester_pubkey: String,
    /// When the attestation was made.
    pub attested_at: DateTime<Utc>,
    /// Cryptographic signature.
    pub signature: String,
}

impl ReVerificationAttestation {
    /// Create a new attestation.
    pub fn new(attester_pubkey: impl Into<String>) -> Self {
        Self {
            attester_pubkey: attester_pubkey.into(),
            attested_at: Utc::now(),
            signature: String::new(),
        }
    }

    /// Set the cryptographic signature.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = signature.into();
        self
    }
}

/// Requirements for attestations in a re-verification session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttestationRequirements {
    /// Minimum number of attestations needed.
    pub min_attestations: usize,
    /// Hours before the session expires.
    pub expiry_hours: u64,
    /// Whether all attesters must be different people.
    pub require_different_attesters: bool,
}

impl Default for AttestationRequirements {
    fn default() -> Self {
        Self {
            min_attestations: 2,
            expiry_hours: 168, // 7 days
            require_different_attesters: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_attestation() {
        let attestation = ReVerificationAttestation::new("alice");
        assert_eq!(attestation.attester_pubkey, "alice");
        assert!(attestation.signature.is_empty());
    }

    #[test]
    fn attestation_with_signature() {
        let attestation = ReVerificationAttestation::new("alice").with_signature("sig_abc");
        assert_eq!(attestation.signature, "sig_abc");
    }

    #[test]
    fn default_requirements() {
        let req = AttestationRequirements::default();
        assert_eq!(req.min_attestations, 2);
        assert_eq!(req.expiry_hours, 168);
        assert!(req.require_different_attesters);
    }

    #[test]
    fn attestation_serialization_roundtrip() {
        let attestation = ReVerificationAttestation::new("alice");
        let json = serde_json::to_string(&attestation).unwrap();
        let deserialized: ReVerificationAttestation = serde_json::from_str(&json).unwrap();
        assert_eq!(attestation, deserialized);
    }
}
