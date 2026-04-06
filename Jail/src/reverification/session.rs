//! Re-verification sessions — identity update workflow.
//!
//! Port of AuthBook's re-verification state machine. A person requests
//! re-verification, collects attestations from others, and completes
//! when enough attestations are gathered within the expiry window.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::JailConfig;
use crate::error::JailError;
use super::attestation::ReVerificationAttestation;

/// A re-verification session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReVerificationSession {
    /// Unique session identifier.
    pub id: Uuid,
    /// The person re-verifying.
    pub pubkey: String,
    /// Why they're re-verifying.
    pub reason: ReVerificationReason,
    /// Collected attestations.
    pub attestations: Vec<ReVerificationAttestation>,
    /// How many attestations are required.
    pub required_attestations: usize,
    /// Current workflow state.
    pub state: ReVerificationState,
    /// When the session started.
    pub started_at: DateTime<Utc>,
    /// When the session was completed (if ever).
    pub completed_at: Option<DateTime<Utc>>,
    /// When the session expires.
    pub expires_at: DateTime<Utc>,
}

/// Why a re-verification is being requested.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReVerificationReason {
    /// Existing verification is outdated.
    VerificationOutdated,
    /// Community flagged verification as inaccurate.
    FlaggedAsInaccurate,
    /// Recovering access to an account.
    AccountRecovery,
    /// Community requires periodic re-verification.
    CommunityRequested,
    /// Voluntary update by the person.
    VoluntaryUpdate,
}

impl std::fmt::Display for ReVerificationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VerificationOutdated => write!(f, "verification_outdated"),
            Self::FlaggedAsInaccurate => write!(f, "flagged_as_inaccurate"),
            Self::AccountRecovery => write!(f, "account_recovery"),
            Self::CommunityRequested => write!(f, "community_requested"),
            Self::VoluntaryUpdate => write!(f, "voluntary_update"),
        }
    }
}

/// State machine for re-verification workflow.
///
/// Pending → Collecting → Completed (or Failed/Expired)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReVerificationState {
    /// Session created, waiting for first attestation.
    Pending,
    /// Collecting attestations.
    Collecting,
    /// Successfully completed (enough attestations gathered).
    Completed,
    /// Explicitly failed (e.g., fraud detected).
    Failed,
    /// Timed out without enough attestations.
    Expired,
}

impl ReVerificationState {
    /// Whether this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Expired
        )
    }
}

impl std::fmt::Display for ReVerificationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Collecting => write!(f, "collecting"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl ReVerificationSession {
    /// Start a new re-verification session.
    pub fn start(
        pubkey: impl Into<String>,
        reason: ReVerificationReason,
        config: &JailConfig,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            reason,
            attestations: Vec::new(),
            required_attestations: config.reverification_attestations_required,
            state: ReVerificationState::Pending,
            started_at: now,
            completed_at: None,
            expires_at: now + Duration::hours(config.reverification_expiry_hours as i64),
        }
    }

    /// Add an attestation from another person.
    pub fn add_attestation(
        &mut self,
        attestation: ReVerificationAttestation,
    ) -> Result<(), JailError> {
        if self.state.is_terminal() {
            return Err(JailError::TerminalState(self.state.to_string()));
        }

        // Check expiry
        if Utc::now() > self.expires_at {
            self.state = ReVerificationState::Expired;
            return Err(JailError::SessionExpired(self.id.to_string()));
        }

        // Check for duplicate attester
        if self
            .attestations
            .iter()
            .any(|a| a.attester_pubkey == attestation.attester_pubkey)
        {
            return Err(JailError::DuplicateAttester(
                attestation.attester_pubkey.clone(),
            ));
        }

        self.attestations.push(attestation);

        // Transition to Collecting if this is the first attestation
        if self.state == ReVerificationState::Pending {
            self.state = ReVerificationState::Collecting;
        }

        Ok(())
    }

    /// Attempt to complete the session.
    pub fn complete(&mut self) -> Result<(), JailError> {
        if self.state.is_terminal() {
            return Err(JailError::AlreadyCompleted(self.id.to_string()));
        }

        if Utc::now() > self.expires_at {
            self.state = ReVerificationState::Expired;
            return Err(JailError::SessionExpired(self.id.to_string()));
        }

        if self.attestations.len() < self.required_attestations {
            return Err(JailError::InsufficientAttestations {
                have: self.attestations.len(),
                need: self.required_attestations,
            });
        }

        self.state = ReVerificationState::Completed;
        self.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Check if the session has expired and update state if so.
    pub fn check_expiry(&mut self) -> bool {
        if !self.state.is_terminal() && Utc::now() > self.expires_at {
            self.state = ReVerificationState::Expired;
            true
        } else {
            false
        }
    }

    /// Explicitly fail the session.
    pub fn fail(&mut self, _reason: &str) -> Result<(), JailError> {
        if self.state.is_terminal() {
            return Err(JailError::TerminalState(self.state.to_string()));
        }
        self.state = ReVerificationState::Failed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> JailConfig {
        JailConfig {
            reverification_attestations_required: 2,
            reverification_expiry_hours: 168,
            ..JailConfig::default()
        }
    }

    fn make_attestation(attester: &str) -> ReVerificationAttestation {
        ReVerificationAttestation::new(attester)
    }

    #[test]
    fn start_session() {
        let config = test_config();
        let session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);
        assert_eq!(session.state, ReVerificationState::Pending);
        assert_eq!(session.required_attestations, 2);
        assert!(session.attestations.is_empty());
    }

    #[test]
    fn add_attestation_transitions_to_collecting() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);

        session.add_attestation(make_attestation("alice")).unwrap();
        assert_eq!(session.state, ReVerificationState::Collecting);
        assert_eq!(session.attestations.len(), 1);
    }

    #[test]
    fn duplicate_attester_rejected() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);

        session.add_attestation(make_attestation("alice")).unwrap();
        let result = session.add_attestation(make_attestation("alice"));
        assert!(matches!(result, Err(JailError::DuplicateAttester(_))));
    }

    #[test]
    fn complete_with_enough_attestations() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);

        session.add_attestation(make_attestation("alice")).unwrap();
        session.add_attestation(make_attestation("carol")).unwrap();
        session.complete().unwrap();
        assert_eq!(session.state, ReVerificationState::Completed);
        assert!(session.completed_at.is_some());
    }

    #[test]
    fn cannot_complete_with_insufficient_attestations() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);

        session.add_attestation(make_attestation("alice")).unwrap();
        let result = session.complete();
        assert!(matches!(
            result,
            Err(JailError::InsufficientAttestations { have: 1, need: 2 })
        ));
    }

    #[test]
    fn expired_session_rejects_attestations() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);
        // Force expiry
        session.expires_at = Utc::now() - Duration::hours(1);

        let result = session.add_attestation(make_attestation("alice"));
        assert!(matches!(result, Err(JailError::SessionExpired(_))));
        assert_eq!(session.state, ReVerificationState::Expired);
    }

    #[test]
    fn fail_session() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::AccountRecovery, &config);

        session.fail("fraud detected").unwrap();
        assert_eq!(session.state, ReVerificationState::Failed);
    }

    #[test]
    fn cannot_add_to_terminal_session() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);
        session.fail("reason").unwrap();

        let result = session.add_attestation(make_attestation("alice"));
        assert!(matches!(result, Err(JailError::TerminalState(_))));
    }

    #[test]
    fn check_expiry() {
        let config = test_config();
        let mut session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);
        assert!(!session.check_expiry());

        session.expires_at = Utc::now() - Duration::hours(1);
        assert!(session.check_expiry());
        assert_eq!(session.state, ReVerificationState::Expired);
    }

    #[test]
    fn terminal_states() {
        assert!(ReVerificationState::Completed.is_terminal());
        assert!(ReVerificationState::Failed.is_terminal());
        assert!(ReVerificationState::Expired.is_terminal());
        assert!(!ReVerificationState::Pending.is_terminal());
        assert!(!ReVerificationState::Collecting.is_terminal());
    }

    #[test]
    fn session_serialization_roundtrip() {
        let config = test_config();
        let session = ReVerificationSession::start("bob", ReVerificationReason::VoluntaryUpdate, &config);
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: ReVerificationSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session, deserialized);
    }
}
