//! Physical exchange protocol — two people meet, verify proximity, dual-sign.
//!
//! A Handoff is a structured exchange between two people who are physically
//! present. It follows a strict lifecycle:
//!
//! ```text
//! Initiated → ProximityVerified → InitiatorSigned → FullySigned → Completed
//!                                                                     ↓
//!     any non-Completed state → Cancelled                         Disputed
//! ```
//!
//! The proximity proof is stripped of raw sensor data (RSSI, NFC tokens,
//! ultrasonic responses) before storage — `ProximityProofRef` captures the
//! method and timestamp without surveillance artifacts.
//!
//! ## Covenant Alignment
//!
//! **Sovereignty** — both parties must sign; no unilateral completion.
//! **Consent** — proximity verification is opt-in and evidence is stripped.
//! **Dignity** — dispute resolution exists; completed handoffs are not final.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::PhysicalError;

// ---------------------------------------------------------------------------
// MARK: - ProximityProofRef
// ---------------------------------------------------------------------------

/// A privacy-stripped reference to a Bulwark ProximityProof.
///
/// Stores ONLY the method used and when it was verified — no RSSI values,
/// no NFC tokens, no ultrasonic data. This is deliberate: the handoff
/// needs to know proximity was verified, not how strong the signal was.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProximityProofRef {
    /// The proximity method used (e.g. "Ble", "Nfc", "Ultrasonic", "Multiple").
    pub method: String,
    /// When the proximity was verified.
    pub verified_at: DateTime<Utc>,
    /// Whether the original proof had any proximity evidence.
    pub evidence_present: bool,
}

impl ProximityProofRef {
    /// Create a stripped reference from a Bulwark ProximityProof.
    ///
    /// Determines the method from which evidence fields are present:
    /// - Multiple fields present → "Multiple"
    /// - Only BLE RSSI → "Ble"
    /// - Only NFC token → "Nfc"
    /// - Only ultrasonic → "Ultrasonic"
    /// - None present → "None"
    pub fn from_bulwark(proof: &bulwark::verification::proximity::ProximityProof) -> Self {
        let has_ble = proof.ble_rssi.is_some();
        let has_nfc = proof.nfc_token.is_some();
        let has_ultrasonic = proof.ultrasonic_response.is_some();

        let count = [has_ble, has_nfc, has_ultrasonic]
            .iter()
            .filter(|&&v| v)
            .count();

        let method = match count {
            0 => "None".to_string(),
            1 => {
                if has_ble {
                    "Ble".to_string()
                } else if has_nfc {
                    "Nfc".to_string()
                } else {
                    "Ultrasonic".to_string()
                }
            }
            _ => "Multiple".to_string(),
        };

        Self {
            method,
            verified_at: proof.nonce_created_at,
            evidence_present: proof.has_proximity_evidence(),
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - HandoffPurpose
// ---------------------------------------------------------------------------

/// Why the handoff is happening.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HandoffPurpose {
    /// Exchanging Cash bearer instruments.
    CashExchange,
    /// Delivering physical goods.
    GoodsDelivery,
    /// In-person identity or document verification.
    Verification,
    /// Community-level attestation (e.g. witness a signing).
    CommunityAttestation,
    /// Free-form purpose.
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - HandoffItemType
// ---------------------------------------------------------------------------

/// The kind of item being exchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HandoffItemType {
    /// A Fortune Cash bearer note.
    CashNote,
    /// A physical good.
    PhysicalGood,
    /// A document (physical or digital).
    Document,
    /// A verification or attestation token.
    VerificationToken,
    /// Free-form item type.
    Custom(String),
}

// ---------------------------------------------------------------------------
// MARK: - HandoffItem
// ---------------------------------------------------------------------------

/// An item being exchanged in a handoff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HandoffItem {
    /// What kind of item this is.
    pub item_type: HandoffItemType,
    /// Human-readable description.
    pub description: String,
    /// Optional reference (Cash serial number, Idea ID, tracking number, etc.).
    pub reference_id: Option<String>,
}

impl HandoffItem {
    /// Create a new handoff item.
    pub fn new(item_type: HandoffItemType, description: impl Into<String>) -> Self {
        Self {
            item_type,
            description: description.into(),
            reference_id: None,
        }
    }

    /// Attach a reference ID (serial number, Idea ID, etc.).
    pub fn with_reference(mut self, reference_id: impl Into<String>) -> Self {
        self.reference_id = Some(reference_id.into());
        self
    }
}

// ---------------------------------------------------------------------------
// MARK: - HandoffStatus
// ---------------------------------------------------------------------------

/// The current state of a handoff.
///
/// Lifecycle: Initiated → ProximityVerified → InitiatorSigned → FullySigned → Completed.
/// Any non-Completed state → Cancelled. Completed → Disputed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HandoffStatus {
    /// Handoff created, waiting for proximity verification.
    Initiated,
    /// Proximity verified, waiting for initiator signature.
    ProximityVerified,
    /// Initiator has signed, waiting for counterparty.
    InitiatorSigned,
    /// Both parties have signed.
    FullySigned,
    /// Exchange completed.
    Completed,
    /// A dispute has been raised after completion.
    Disputed,
    /// Cancelled before completion.
    Cancelled,
}

impl std::fmt::Display for HandoffStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandoffStatus::Initiated => write!(f, "Initiated"),
            HandoffStatus::ProximityVerified => write!(f, "ProximityVerified"),
            HandoffStatus::InitiatorSigned => write!(f, "InitiatorSigned"),
            HandoffStatus::FullySigned => write!(f, "FullySigned"),
            HandoffStatus::Completed => write!(f, "Completed"),
            HandoffStatus::Disputed => write!(f, "Disputed"),
            HandoffStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

// ---------------------------------------------------------------------------
// MARK: - Handoff
// ---------------------------------------------------------------------------

/// A physical exchange between two people.
///
/// Follows a strict state machine: proximity must be verified before
/// signing, the initiator must sign before the counterparty, and both
/// must sign before completion. Cancellation is available from any
/// non-Completed state. Disputes can only be raised after completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Handoff {
    pub id: Uuid,
    pub initiator: String,
    pub counterparty: String,
    pub purpose: HandoffPurpose,
    pub proximity_proof: Option<ProximityProofRef>,
    pub items: Vec<HandoffItem>,
    pub initiator_sig: Option<String>,
    pub counterparty_sig: Option<String>,
    pub status: HandoffStatus,
    pub rendezvous_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

impl Handoff {
    /// Create a new handoff between two parties.
    ///
    /// Errors if `initiator == counterparty` (cannot handoff with yourself).
    pub fn new(
        initiator: impl Into<String>,
        counterparty: impl Into<String>,
        purpose: HandoffPurpose,
    ) -> Result<Self, PhysicalError> {
        let initiator = initiator.into();
        let counterparty = counterparty.into();

        if initiator == counterparty {
            return Err(PhysicalError::SelfHandoff);
        }

        Ok(Self {
            id: Uuid::new_v4(),
            initiator,
            counterparty,
            purpose,
            proximity_proof: None,
            items: Vec::new(),
            initiator_sig: None,
            counterparty_sig: None,
            status: HandoffStatus::Initiated,
            rendezvous_id: None,
            created_at: Utc::now(),
            completed_at: None,
            notes: None,
        })
    }

    // -- Builder methods ----------------------------------------------------

    /// Set the items to be exchanged.
    pub fn with_items(mut self, items: Vec<HandoffItem>) -> Self {
        self.items = items;
        self
    }

    /// Link this handoff to a rendezvous.
    pub fn with_rendezvous(mut self, id: Uuid) -> Self {
        self.rendezvous_id = Some(id);
        self
    }

    /// Attach notes to the handoff.
    pub fn with_notes(mut self, text: impl Into<String>) -> Self {
        self.notes = Some(text.into());
        self
    }

    // -- Item management ----------------------------------------------------

    /// Add an item to the exchange.
    pub fn add_item(&mut self, item: HandoffItem) {
        self.items.push(item);
    }

    /// Convenience: add a Cash note with serial and description.
    pub fn add_cash_note(&mut self, serial: impl Into<String>, description: impl Into<String>) {
        self.items.push(
            HandoffItem::new(HandoffItemType::CashNote, description)
                .with_reference(serial),
        );
    }

    // -- State transitions --------------------------------------------------

    /// Record proximity verification.
    ///
    /// Can only transition from Initiated.
    pub fn verify_proximity(&mut self, proof_ref: ProximityProofRef) -> Result<(), PhysicalError> {
        if self.status != HandoffStatus::Initiated {
            return Err(PhysicalError::InvalidHandoffState {
                expected: "Initiated".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.proximity_proof = Some(proof_ref);
        self.status = HandoffStatus::ProximityVerified;
        Ok(())
    }

    /// Record the initiator's signature.
    ///
    /// Requires proximity to be verified first (ProximityVerified state).
    pub fn sign_initiator(&mut self, signature: &str) -> Result<(), PhysicalError> {
        if self.status != HandoffStatus::ProximityVerified {
            if self.status == HandoffStatus::Initiated {
                return Err(PhysicalError::ProximityRequired);
            }
            return Err(PhysicalError::InvalidHandoffState {
                expected: "ProximityVerified".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.initiator_sig = Some(signature.to_string());
        self.status = HandoffStatus::InitiatorSigned;
        Ok(())
    }

    /// Record the counterparty's signature.
    ///
    /// Requires the initiator to have signed first (InitiatorSigned state).
    pub fn sign_counterparty(&mut self, signature: &str) -> Result<(), PhysicalError> {
        if self.status != HandoffStatus::InitiatorSigned {
            if self.status == HandoffStatus::ProximityVerified
                || self.status == HandoffStatus::Initiated
            {
                return Err(PhysicalError::InitiatorSignatureRequired);
            }
            return Err(PhysicalError::InvalidHandoffState {
                expected: "InitiatorSigned".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.counterparty_sig = Some(signature.to_string());
        self.status = HandoffStatus::FullySigned;
        Ok(())
    }

    /// Complete the handoff.
    ///
    /// Requires both signatures (FullySigned state).
    pub fn complete(&mut self) -> Result<(), PhysicalError> {
        if self.status != HandoffStatus::FullySigned {
            return Err(PhysicalError::InvalidHandoffState {
                expected: "FullySigned".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.status = HandoffStatus::Completed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Cancel the handoff.
    ///
    /// Available from any state except Completed (and Disputed, which is
    /// post-Completed).
    pub fn cancel(&mut self) -> Result<(), PhysicalError> {
        if self.status == HandoffStatus::Completed || self.status == HandoffStatus::Disputed {
            return Err(PhysicalError::InvalidHandoffState {
                expected: "any non-Completed state".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.status = HandoffStatus::Cancelled;
        Ok(())
    }

    /// Raise a dispute on a completed handoff.
    ///
    /// Only available from the Completed state. Stores the reason in notes.
    pub fn dispute(&mut self, reason: &str) -> Result<(), PhysicalError> {
        if self.status != HandoffStatus::Completed {
            return Err(PhysicalError::InvalidHandoffState {
                expected: "Completed".to_string(),
                actual: self.status.to_string(),
            });
        }
        self.notes = Some(reason.to_string());
        self.status = HandoffStatus::Disputed;
        Ok(())
    }

    // -- Queries ------------------------------------------------------------

    /// Whether the handoff has been completed.
    pub fn is_complete(&self) -> bool {
        self.status == HandoffStatus::Completed
    }

    /// Produce deterministic bytes for signing.
    ///
    /// Serializes (id, initiator, counterparty, purpose, items, rendezvous_id)
    /// as canonical JSON. Signatures and status are excluded — they depend
    /// on the bytes, not the other way around.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let signable = serde_json::json!({
            "id": self.id,
            "initiator": self.initiator,
            "counterparty": self.counterparty,
            "purpose": self.purpose,
            "items": self.items,
            "rendezvous_id": self.rendezvous_id,
        });
        serde_json::to_vec(&signable).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// MARK: - Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Construction -------------------------------------------------------

    #[test]
    fn new_handoff_creates_initiated() {
        let h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        assert_eq!(h.status, HandoffStatus::Initiated);
        assert_eq!(h.initiator, "cpub1alice");
        assert_eq!(h.counterparty, "cpub1bob");
        assert!(h.items.is_empty());
        assert!(h.initiator_sig.is_none());
        assert!(h.counterparty_sig.is_none());
        assert!(h.proximity_proof.is_none());
        assert!(h.completed_at.is_none());
    }

    #[test]
    fn self_handoff_rejected() {
        let result = Handoff::new("cpub1alice", "cpub1alice", HandoffPurpose::Verification);
        assert_eq!(result.unwrap_err(), PhysicalError::SelfHandoff);
    }

    // -- Builder methods ----------------------------------------------------

    #[test]
    fn with_items_sets_items() {
        let items = vec![
            HandoffItem::new(HandoffItemType::CashNote, "100 Cool note")
                .with_reference("SN-001"),
            HandoffItem::new(HandoffItemType::PhysicalGood, "Handmade mug"),
        ];
        let h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::GoodsDelivery)
            .unwrap()
            .with_items(items.clone());
        assert_eq!(h.items.len(), 2);
        assert_eq!(h.items[0].reference_id.as_deref(), Some("SN-001"));
    }

    #[test]
    fn with_rendezvous_sets_id() {
        let rv_id = Uuid::new_v4();
        let h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::Verification)
            .unwrap()
            .with_rendezvous(rv_id);
        assert_eq!(h.rendezvous_id, Some(rv_id));
    }

    #[test]
    fn with_notes_sets_text() {
        let h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange)
            .unwrap()
            .with_notes("Bring exact change");
        assert_eq!(h.notes.as_deref(), Some("Bring exact change"));
    }

    #[test]
    fn add_cash_note_convenience() {
        let mut h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        h.add_cash_note("SN-42", "50 Cool bearer note");
        assert_eq!(h.items.len(), 1);
        assert_eq!(h.items[0].item_type, HandoffItemType::CashNote);
        assert_eq!(h.items[0].reference_id.as_deref(), Some("SN-42"));
    }

    // -- Happy-path lifecycle -----------------------------------------------

    #[test]
    fn full_lifecycle_happy_path() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();

        // Step 1: verify proximity
        let proof_ref = ProximityProofRef {
            method: "Ble".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        assert!(h.verify_proximity(proof_ref).is_ok());
        assert_eq!(h.status, HandoffStatus::ProximityVerified);

        // Step 2: initiator signs
        assert!(h.sign_initiator("deadbeef01").is_ok());
        assert_eq!(h.status, HandoffStatus::InitiatorSigned);

        // Step 3: counterparty signs
        assert!(h.sign_counterparty("cafebabe02").is_ok());
        assert_eq!(h.status, HandoffStatus::FullySigned);

        // Step 4: complete
        assert!(h.complete().is_ok());
        assert_eq!(h.status, HandoffStatus::Completed);
        assert!(h.is_complete());
        assert!(h.completed_at.is_some());
    }

    // -- State machine enforcement ------------------------------------------

    #[test]
    fn cannot_sign_without_proximity() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let result = h.sign_initiator("sig");
        assert_eq!(result.unwrap_err(), PhysicalError::ProximityRequired);
    }

    #[test]
    fn counterparty_cannot_sign_before_initiator() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let proof_ref = ProximityProofRef {
            method: "Nfc".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        h.verify_proximity(proof_ref).unwrap();

        let result = h.sign_counterparty("sig");
        assert_eq!(
            result.unwrap_err(),
            PhysicalError::InitiatorSignatureRequired
        );
    }

    #[test]
    fn cannot_complete_without_full_signatures() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let result = h.complete();
        assert!(matches!(
            result,
            Err(PhysicalError::InvalidHandoffState { .. })
        ));
    }

    #[test]
    fn cannot_verify_proximity_twice() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let proof_ref = ProximityProofRef {
            method: "Ble".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        h.verify_proximity(proof_ref.clone()).unwrap();
        let result = h.verify_proximity(proof_ref);
        assert!(matches!(
            result,
            Err(PhysicalError::InvalidHandoffState { .. })
        ));
    }

    // -- Cancellation -------------------------------------------------------

    #[test]
    fn cancel_from_initiated() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        assert!(h.cancel().is_ok());
        assert_eq!(h.status, HandoffStatus::Cancelled);
    }

    #[test]
    fn cancel_from_proximity_verified() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let proof_ref = ProximityProofRef {
            method: "Ble".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        h.verify_proximity(proof_ref).unwrap();
        assert!(h.cancel().is_ok());
        assert_eq!(h.status, HandoffStatus::Cancelled);
    }

    #[test]
    fn cannot_cancel_completed() {
        let mut h = make_completed_handoff();
        let result = h.cancel();
        assert!(matches!(
            result,
            Err(PhysicalError::InvalidHandoffState { .. })
        ));
    }

    // -- Disputes -----------------------------------------------------------

    #[test]
    fn dispute_from_completed() {
        let mut h = make_completed_handoff();
        assert!(h.dispute("Items were defective").is_ok());
        assert_eq!(h.status, HandoffStatus::Disputed);
        assert_eq!(h.notes.as_deref(), Some("Items were defective"));
    }

    #[test]
    fn cannot_dispute_non_completed() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let result = h.dispute("No reason");
        assert!(matches!(
            result,
            Err(PhysicalError::InvalidHandoffState { .. })
        ));
    }

    // -- Signable bytes -----------------------------------------------------

    #[test]
    fn signable_bytes_deterministic() {
        let h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let bytes1 = h.signable_bytes();
        let bytes2 = h.signable_bytes();
        assert_eq!(bytes1, bytes2);
        assert!(!bytes1.is_empty());
    }

    #[test]
    fn signable_bytes_exclude_signatures() {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let bytes_before = h.signable_bytes();

        let proof_ref = ProximityProofRef {
            method: "Ble".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        h.verify_proximity(proof_ref).unwrap();
        h.sign_initiator("deadbeef").unwrap();

        let bytes_after = h.signable_bytes();
        // Signable content hasn't changed — signatures and status are excluded.
        assert_eq!(bytes_before, bytes_after);
    }

    // -- ProximityProofRef from Bulwark -------------------------------------

    #[test]
    fn proof_ref_from_ble_only() {
        let proof = bulwark::verification::proximity::ProximityProof::new("nonce1")
            .with_ble(-45);
        let ref_ = ProximityProofRef::from_bulwark(&proof);
        assert_eq!(ref_.method, "Ble");
        assert!(ref_.evidence_present);
    }

    #[test]
    fn proof_ref_from_nfc_only() {
        let proof = bulwark::verification::proximity::ProximityProof::new("nonce2")
            .with_nfc("token123");
        let ref_ = ProximityProofRef::from_bulwark(&proof);
        assert_eq!(ref_.method, "Nfc");
        assert!(ref_.evidence_present);
    }

    #[test]
    fn proof_ref_from_multiple() {
        let proof = bulwark::verification::proximity::ProximityProof::new("nonce3")
            .with_ble(-40)
            .with_nfc("token");
        let ref_ = ProximityProofRef::from_bulwark(&proof);
        assert_eq!(ref_.method, "Multiple");
        assert!(ref_.evidence_present);
    }

    #[test]
    fn proof_ref_from_no_evidence() {
        let proof = bulwark::verification::proximity::ProximityProof::new("nonce4");
        let ref_ = ProximityProofRef::from_bulwark(&proof);
        assert_eq!(ref_.method, "None");
        assert!(!ref_.evidence_present);
    }

    // -- Serde round-trip ---------------------------------------------------

    #[test]
    fn handoff_serde_round_trip() {
        let mut h = Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange)
            .unwrap()
            .with_notes("Test handoff");
        h.add_cash_note("SN-100", "100 Cool note");

        let json = serde_json::to_string(&h).unwrap();
        let parsed: Handoff = serde_json::from_str(&json).unwrap();
        assert_eq!(h, parsed);
    }

    // -- Helpers ------------------------------------------------------------

    /// Create a fully completed handoff for testing post-completion states.
    fn make_completed_handoff() -> Handoff {
        let mut h =
            Handoff::new("cpub1alice", "cpub1bob", HandoffPurpose::CashExchange).unwrap();
        let proof_ref = ProximityProofRef {
            method: "Ble".to_string(),
            verified_at: Utc::now(),
            evidence_present: true,
        };
        h.verify_proximity(proof_ref).unwrap();
        h.sign_initiator("sig1").unwrap();
        h.sign_counterparty("sig2").unwrap();
        h.complete().unwrap();
        h
    }
}
