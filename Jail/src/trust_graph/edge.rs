//! Verification edges — the atoms of the trust graph.
//!
//! Each edge records one person verifying another. The method is a string ID
//! (from Bulwark's VerificationMethod trait), keeping Jail decoupled from
//! any specific verification implementation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A directed edge in the trust graph: one person verified another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationEdge {
    /// Unique edge identifier.
    pub id: Uuid,
    /// Pubkey of the person doing the verifying.
    pub verifier_pubkey: String,
    /// Pubkey of the person being verified.
    pub verified_pubkey: String,
    /// Verification method ID (from Bulwark's VerificationMethod trait).
    pub method: String,
    /// How the verifier feels about this verification.
    pub sentiment: VerificationSentiment,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// When the verification occurred.
    pub verified_at: DateTime<Utc>,
    /// Cryptographic signature of the verification record.
    pub signature: String,
}

impl VerificationEdge {
    /// Create a new verification edge.
    pub fn new(
        verifier_pubkey: impl Into<String>,
        verified_pubkey: impl Into<String>,
        method: impl Into<String>,
        sentiment: VerificationSentiment,
        confidence: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            verifier_pubkey: verifier_pubkey.into(),
            verified_pubkey: verified_pubkey.into(),
            method: method.into(),
            sentiment,
            confidence: confidence.clamp(0.0, 1.0),
            verified_at: Utc::now(),
            signature: String::new(),
        }
    }

    /// Set the cryptographic signature.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = signature.into();
        self
    }
}

/// How the verifier feels about the verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerificationSentiment {
    /// Fully confident in identity.
    Positive,
    /// Verified but neutral.
    Neutral,
    /// Some doubts or concerns.
    Cautious,
}

impl std::fmt::Display for VerificationSentiment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Positive => write!(f, "positive"),
            Self::Neutral => write!(f, "neutral"),
            Self::Cautious => write!(f, "cautious"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_edge() {
        let edge = VerificationEdge::new(
            "alice",
            "bob",
            "mutual_vouch",
            VerificationSentiment::Positive,
            0.9,
        );
        assert_eq!(edge.verifier_pubkey, "alice");
        assert_eq!(edge.verified_pubkey, "bob");
        assert_eq!(edge.method, "mutual_vouch");
        assert_eq!(edge.sentiment, VerificationSentiment::Positive);
        assert!((edge.confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_clamped() {
        let edge = VerificationEdge::new("a", "b", "m", VerificationSentiment::Neutral, 1.5);
        assert!((edge.confidence - 1.0).abs() < f64::EPSILON);

        let edge = VerificationEdge::new("a", "b", "m", VerificationSentiment::Neutral, -0.5);
        assert!(edge.confidence.abs() < f64::EPSILON);
    }

    #[test]
    fn with_signature() {
        let edge = VerificationEdge::new("a", "b", "m", VerificationSentiment::Positive, 0.8)
            .with_signature("sig123");
        assert_eq!(edge.signature, "sig123");
    }

    #[test]
    fn sentiment_display() {
        assert_eq!(VerificationSentiment::Positive.to_string(), "positive");
        assert_eq!(VerificationSentiment::Neutral.to_string(), "neutral");
        assert_eq!(VerificationSentiment::Cautious.to_string(), "cautious");
    }

    #[test]
    fn edge_serialization_roundtrip() {
        let edge = VerificationEdge::new("alice", "bob", "proximity", VerificationSentiment::Positive, 0.95);
        let json = serde_json::to_string(&edge).unwrap();
        let deserialized: VerificationEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(edge, deserialized);
    }
}
