use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::proposal::ProposalType;

/// A mandate carried by a delegate — specific scope, not general authority.
///
/// From Constellation Art. 8 §3: "Communities may send delegates to higher
/// coordinating bodies carrying specific mandates, not general authority.
/// Such delegates shall carry written mandates specifying their decision-making scope,
/// serve strictly limited terms with immediate recall."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mandate {
    pub id: Uuid,
    pub delegate_pubkey: String,
    pub granting_community: Uuid,
    pub serves_consortium: Uuid,
    pub authorized_decisions: Vec<ProposalType>,
    pub requires_consultation: Vec<ProposalType>,
    pub prohibited_decisions: Vec<ProposalType>,
    pub additional_constraints: Vec<String>,
    pub term_start: DateTime<Utc>,
    pub term_end: Option<DateTime<Utc>>,
    pub recalled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Mandate {
    pub fn new(
        delegate_pubkey: impl Into<String>,
        granting_community: Uuid,
        serves_consortium: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegate_pubkey: delegate_pubkey.into(),
            granting_community,
            serves_consortium,
            authorized_decisions: Vec::new(),
            requires_consultation: Vec::new(),
            prohibited_decisions: Vec::new(),
            additional_constraints: Vec::new(),
            term_start: Utc::now(),
            term_end: None,
            recalled_at: None,
            created_at: Utc::now(),
        }
    }

    /// Standard mandate: authorized for standard/policy, requires consultation
    /// for treasury/amendment/federation, prohibited from dissolution.
    pub fn standard(
        delegate_pubkey: impl Into<String>,
        granting_community: Uuid,
        serves_consortium: Uuid,
    ) -> Self {
        Self {
            authorized_decisions: vec![
                ProposalType::Standard,
                ProposalType::Policy,
                ProposalType::Membership,
            ],
            requires_consultation: vec![
                ProposalType::Treasury,
                ProposalType::Amendment,
                ProposalType::Federation,
            ],
            prohibited_decisions: vec![ProposalType::Emergency],
            ..Self::new(delegate_pubkey, granting_community, serves_consortium)
        }
    }

    /// Limited mandate: authorized for standard only, consults on everything else.
    pub fn limited(
        delegate_pubkey: impl Into<String>,
        granting_community: Uuid,
        serves_consortium: Uuid,
    ) -> Self {
        Self {
            authorized_decisions: vec![ProposalType::Standard],
            requires_consultation: vec![
                ProposalType::Policy,
                ProposalType::Treasury,
                ProposalType::Membership,
            ],
            prohibited_decisions: vec![
                ProposalType::Amendment,
                ProposalType::Federation,
                ProposalType::Emergency,
            ],
            ..Self::new(delegate_pubkey, granting_community, serves_consortium)
        }
    }

    /// Set when this mandate expires (builder pattern).
    pub fn with_term_end(mut self, end: DateTime<Utc>) -> Self {
        self.term_end = Some(end);
        self
    }

    /// Whether this mandate is still in effect (not recalled, not expired).
    pub fn is_active(&self) -> bool {
        self.recalled_at.is_none() && !self.is_term_ended()
    }

    /// Whether the mandate's term has passed its end date.
    pub fn is_term_ended(&self) -> bool {
        self.term_end
            .is_some_and(|end| Utc::now() > end)
    }

    /// Whether this mandate has been recalled by the granting community.
    pub fn is_recalled(&self) -> bool {
        self.recalled_at.is_some()
    }

    /// Check if this mandate authorizes a decision type.
    pub fn can_decide(&self, decision_type: &ProposalType) -> MandateDecision {
        if self.prohibited_decisions.contains(decision_type) {
            MandateDecision::Prohibited
        } else if self.requires_consultation.contains(decision_type) {
            MandateDecision::RequiresConsultation
        } else if self.authorized_decisions.contains(decision_type) {
            MandateDecision::Authorized
        } else {
            MandateDecision::RequiresConsultation // default: consult
        }
    }
}

/// Whether a mandate authorizes a given decision type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MandateDecision {
    /// The delegate may act independently on this decision type.
    Authorized,
    /// The delegate must consult their community before deciding.
    RequiresConsultation,
    /// The delegate may not act on this decision type at all.
    Prohibited,
}

/// A delegate representing a community in a consortium.
///
/// From Constellation Art. 8 §3: "No delegate may exceed their mandate or
/// speak beyond their community's position."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Delegate {
    pub id: Uuid,
    pub pubkey: String,
    pub represents_community: Uuid,
    pub serves_consortium: Uuid,
    pub mandate: Mandate,
    pub appointed_by: AppointmentSource,
    pub activity_log: Vec<DelegateActivity>,
    pub recallable: bool,
    pub appointed_at: DateTime<Utc>,
}

impl Delegate {
    pub fn new(
        pubkey: impl Into<String>,
        represents_community: Uuid,
        serves_consortium: Uuid,
        mandate: Mandate,
        appointed_by: AppointmentSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            represents_community,
            serves_consortium,
            mandate,
            appointed_by,
            activity_log: Vec::new(),
            recallable: true,
            appointed_at: Utc::now(),
        }
    }

    /// Whether this delegate's mandate is still active.
    pub fn is_active(&self) -> bool {
        self.mandate.is_active()
    }

    /// Record an activity for accountability tracking.
    pub fn log_activity(&mut self, activity: DelegateActivity) {
        self.activity_log.push(activity);
    }
}

/// How a delegate was appointed.
///
/// From Constellation Art. 8 §6: "Strict single-term limits for all coordinating
/// positions, prohibition on consecutive service in similar roles."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppointmentSource {
    /// Elected by community vote (carries the proposal ID).
    Election(Uuid),
    /// Appointed by rotation (carries the rotation round number).
    Rotation(u32),
    /// Selected by lottery (carries the randomness seed or proof).
    Lottery(String),
    /// Chosen by community consensus.
    Consensus,
    /// Appointed by another delegate (carries the appointer's pubkey).
    DelegateAppointment(String),
}

/// A record of delegate activity (for accountability).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegateActivity {
    pub id: Uuid,
    pub delegate_id: Uuid,
    pub activity_type: DelegateActivityType,
    pub proposal_id: Option<Uuid>,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
}

impl DelegateActivity {
    pub fn new(
        delegate_id: Uuid,
        activity_type: DelegateActivityType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegate_id,
            activity_type,
            proposal_id: None,
            description: description.into(),
            occurred_at: Utc::now(),
        }
    }
}

/// Types of delegate activity.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DelegateActivityType {
    /// Cast a vote on a proposal.
    VoteCast,
    /// Submitted a new proposal.
    ProposalSubmitted,
    /// Contributed to a proposal discussion.
    DiscussionContributed,
    /// Initiated a consultation with their community.
    ConsultationInitiated,
    /// Completed a community consultation.
    ConsultationCompleted,
    /// Attended an assembly or convocation.
    AttendedAssembly,
    /// Failed to attend a scheduled assembly.
    MissedAssembly,
}

/// A recall motion against a delegate.
///
/// From Constellation Art. 8 §3: "serve strictly limited terms with immediate recall."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegateRecall {
    pub id: Uuid,
    pub delegate_id: Uuid,
    pub delegate_pubkey: String,
    pub community_id: Uuid,
    pub reason: String,
    pub proposal_id: Option<Uuid>,
    pub signatures_required: u32,
    pub signatures: Vec<RecallSignature>,
    pub status: RecallStatus,
    pub initiated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl DelegateRecall {
    pub fn new(
        delegate_id: Uuid,
        delegate_pubkey: impl Into<String>,
        community_id: Uuid,
        reason: impl Into<String>,
        signatures_required: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            delegate_id,
            delegate_pubkey: delegate_pubkey.into(),
            community_id,
            reason: reason.into(),
            proposal_id: None,
            signatures_required,
            signatures: Vec::new(),
            status: RecallStatus::Collecting,
            initiated_at: Utc::now(),
            resolved_at: None,
        }
    }

    /// Add a signature to the recall petition. Auto-triggers when threshold is met.
    pub fn add_signature(&mut self, pubkey: impl Into<String>, signature: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.has_signed(&pubkey) {
            self.signatures.push(RecallSignature {
                pubkey,
                signature: signature.into(),
                signed_at: Utc::now(),
            });
        }
        // Auto-trigger if threshold met
        if self.signatures.len() as u32 >= self.signatures_required
            && self.status == RecallStatus::Collecting
        {
            self.status = RecallStatus::Triggered;
        }
    }

    /// Whether this pubkey has already signed the recall petition.
    pub fn has_signed(&self, pubkey: &str) -> bool {
        self.signatures.iter().any(|s| s.pubkey == pubkey)
    }

    /// Resolve the recall vote: `true` = delegate recalled, `false` = delegate retained.
    pub fn resolve(&mut self, succeeded: bool) {
        self.status = if succeeded {
            RecallStatus::Recalled
        } else {
            RecallStatus::Retained
        };
        self.resolved_at = Some(Utc::now());
    }
}

/// A signature supporting a recall motion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecallSignature {
    pub pubkey: String,
    pub signature: String,
    pub signed_at: DateTime<Utc>,
}

/// Lifecycle of a recall motion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RecallStatus {
    /// Gathering signatures.
    Collecting,
    /// Enough signatures, recall vote triggered.
    Triggered,
    /// Recall vote in progress.
    Voting,
    /// Delegate was recalled.
    Recalled,
    /// Delegate was retained.
    Retained,
    /// Motion withdrawn.
    Withdrawn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_mandate_permissions() {
        let m = Mandate::standard("alice", Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(m.can_decide(&ProposalType::Standard), MandateDecision::Authorized);
        assert_eq!(m.can_decide(&ProposalType::Policy), MandateDecision::Authorized);
        assert_eq!(
            m.can_decide(&ProposalType::Treasury),
            MandateDecision::RequiresConsultation
        );
        assert_eq!(m.can_decide(&ProposalType::Emergency), MandateDecision::Prohibited);
    }

    #[test]
    fn limited_mandate_permissions() {
        let m = Mandate::limited("alice", Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(m.can_decide(&ProposalType::Standard), MandateDecision::Authorized);
        assert_eq!(
            m.can_decide(&ProposalType::Policy),
            MandateDecision::RequiresConsultation
        );
        assert_eq!(m.can_decide(&ProposalType::Amendment), MandateDecision::Prohibited);
    }

    #[test]
    fn mandate_active_state() {
        let m = Mandate::new("alice", Uuid::new_v4(), Uuid::new_v4());
        assert!(m.is_active());
        assert!(!m.is_recalled());
    }

    #[test]
    fn mandate_expired() {
        let mut m = Mandate::new("alice", Uuid::new_v4(), Uuid::new_v4());
        m.term_end = Some(Utc::now() - chrono::Duration::days(1));
        assert!(!m.is_active());
        assert!(m.is_term_ended());
    }

    #[test]
    fn mandate_recalled() {
        let mut m = Mandate::new("alice", Uuid::new_v4(), Uuid::new_v4());
        m.recalled_at = Some(Utc::now());
        assert!(!m.is_active());
        assert!(m.is_recalled());
    }

    #[test]
    fn delegate_creation_and_activity() {
        let community = Uuid::new_v4();
        let consortium = Uuid::new_v4();
        let mandate = Mandate::standard("alice", community, consortium);
        let mut delegate = Delegate::new(
            "alice",
            community,
            consortium,
            mandate,
            AppointmentSource::Election(Uuid::new_v4()),
        );

        assert!(delegate.is_active());
        assert!(delegate.recallable);

        delegate.log_activity(DelegateActivity::new(
            delegate.id,
            DelegateActivityType::VoteCast,
            "Voted on infrastructure proposal",
        ));
        assert_eq!(delegate.activity_log.len(), 1);
    }

    #[test]
    fn recall_signature_threshold() {
        let mut recall = DelegateRecall::new(
            Uuid::new_v4(),
            "alice",
            Uuid::new_v4(),
            "Exceeded mandate authority",
            3, // 3 signatures required
        );
        assert_eq!(recall.status, RecallStatus::Collecting);

        recall.add_signature("bob", "sig_bob");
        recall.add_signature("charlie", "sig_charlie");
        assert_eq!(recall.status, RecallStatus::Collecting);

        recall.add_signature("bob", "sig_bob_again"); // duplicate ignored
        assert_eq!(recall.signatures.len(), 2);

        recall.add_signature("diana", "sig_diana");
        assert_eq!(recall.status, RecallStatus::Triggered); // auto-triggered

        recall.resolve(true);
        assert_eq!(recall.status, RecallStatus::Recalled);
        assert!(recall.resolved_at.is_some());
    }

    #[test]
    fn recall_retained() {
        let mut recall = DelegateRecall::new(
            Uuid::new_v4(),
            "alice",
            Uuid::new_v4(),
            "Bad vibes",
            2,
        );
        recall.add_signature("bob", "sig");
        recall.add_signature("charlie", "sig");
        recall.resolve(false);
        assert_eq!(recall.status, RecallStatus::Retained);
    }
}
