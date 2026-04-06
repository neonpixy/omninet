use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A pluggable verification method — proximity is ONE option, not the only one.
///
/// Physical proximity is NOT required for adult trust.
/// Multiple verification methods exist. Physical proximity is ONE option, not the only root."
///
/// Built-in methods:
/// - ProximityVerification (BLE/NFC/QR — highest weight)
/// - MutualVouchVerification (two people attest to each other)
/// - CommunitySponsorVerification (established member brings you in)
/// - DigitalAttestationVerification (Crown-verified identity)
/// - ReputationBasedVerification (trust earned over time)
/// - TimeBasedVerification (network age threshold)
pub trait VerificationMethod: Send + Sync {
    /// Unique identifier for this method.
    fn method_id(&self) -> &str;

    /// How much trust weight this method carries (0.0 to 1.0).
    fn trust_weight(&self) -> f64;

    /// Human-readable description.
    fn description(&self) -> &str;
}

/// Evidence submitted for verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationEvidence {
    pub id: Uuid,
    pub method_id: String,
    pub subject_pubkey: String,
    pub evidence_data: std::collections::HashMap<String, String>,
    pub submitted_at: DateTime<Utc>,
}

impl VerificationEvidence {
    pub fn new(method_id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            method_id: method_id.into(),
            subject_pubkey: subject.into(),
            evidence_data: std::collections::HashMap::new(),
            submitted_at: Utc::now(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.evidence_data.insert(key.into(), value.into());
        self
    }
}

/// Result of a verification attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationResult {
    pub verified: bool,
    pub method_id: String,
    pub trust_weight: f64,
    pub verified_at: DateTime<Utc>,
    pub notes: Option<String>,
}

// Built-in implementations

pub struct ProximityVerification;
impl VerificationMethod for ProximityVerification {
    fn method_id(&self) -> &str { "proximity" }
    fn trust_weight(&self) -> f64 { 1.0 }
    fn description(&self) -> &str { "Physical proximity verified via BLE, NFC, or QR" }
}

pub struct MutualVouchVerification;
impl VerificationMethod for MutualVouchVerification {
    fn method_id(&self) -> &str { "mutual_vouch" }
    fn trust_weight(&self) -> f64 { 0.7 }
    fn description(&self) -> &str { "Two people attest to knowing each other" }
}

pub struct CommunitySponsorVerification;
impl VerificationMethod for CommunitySponsorVerification {
    fn method_id(&self) -> &str { "community_sponsor" }
    fn trust_weight(&self) -> f64 { 0.8 }
    fn description(&self) -> &str { "Established community member vouches for newcomer" }
}

pub struct DigitalAttestationVerification;
impl VerificationMethod for DigitalAttestationVerification {
    fn method_id(&self) -> &str { "digital_attestation" }
    fn trust_weight(&self) -> f64 { 0.5 }
    fn description(&self) -> &str { "Crown-verified identity through digital channels" }
}

pub struct ReputationBasedVerification;
impl VerificationMethod for ReputationBasedVerification {
    fn method_id(&self) -> &str { "reputation" }
    fn trust_weight(&self) -> f64 { 0.6 }
    fn description(&self) -> &str { "Trust earned through positive network history" }
}

pub struct TimeBasedVerification;
impl VerificationMethod for TimeBasedVerification {
    fn method_id(&self) -> &str { "time_based" }
    fn trust_weight(&self) -> f64 { 0.4 }
    fn description(&self) -> &str { "Network age exceeds trust threshold" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_methods_have_unique_ids() {
        let methods: Vec<Box<dyn VerificationMethod>> = vec![
            Box::new(ProximityVerification),
            Box::new(MutualVouchVerification),
            Box::new(CommunitySponsorVerification),
            Box::new(DigitalAttestationVerification),
            Box::new(ReputationBasedVerification),
            Box::new(TimeBasedVerification),
        ];
        let ids: std::collections::HashSet<&str> = methods.iter().map(|m| m.method_id()).collect();
        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn proximity_has_highest_weight() {
        let methods: Vec<Box<dyn VerificationMethod>> = vec![
            Box::new(ProximityVerification),
            Box::new(MutualVouchVerification),
            Box::new(CommunitySponsorVerification),
            Box::new(DigitalAttestationVerification),
            Box::new(ReputationBasedVerification),
            Box::new(TimeBasedVerification),
        ];
        let max = methods.iter().max_by(|a, b| {
            a.trust_weight().partial_cmp(&b.trust_weight()).unwrap()
        }).unwrap();
        assert_eq!(max.method_id(), "proximity");
    }

    #[test]
    fn verification_evidence_builder() {
        let evidence = VerificationEvidence::new("proximity", "alice")
            .with_data("ble_rssi", "-45")
            .with_data("nonce", "abc123");
        assert_eq!(evidence.method_id, "proximity");
        assert_eq!(evidence.evidence_data.len(), 2);
    }

    #[test]
    fn all_weights_in_range() {
        let methods: Vec<Box<dyn VerificationMethod>> = vec![
            Box::new(ProximityVerification),
            Box::new(MutualVouchVerification),
            Box::new(CommunitySponsorVerification),
            Box::new(DigitalAttestationVerification),
            Box::new(ReputationBasedVerification),
            Box::new(TimeBasedVerification),
        ];
        for method in &methods {
            let w = method.trust_weight();
            assert!((0.0..=1.0).contains(&w), "{} has weight {w}", method.method_id());
        }
    }
}
