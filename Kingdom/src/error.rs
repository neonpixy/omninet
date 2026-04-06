use thiserror::Error;

/// Errors arising from governance operations within Kingdom.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum KingdomError {
    // Community
    #[error("community not found: {0}")]
    CommunityNotFound(String),

    #[error("community already exists: {0}")]
    CommunityAlreadyExists(String),

    #[error("community is not active: {0}")]
    CommunityNotActive(String),

    #[error("community is dissolved and cannot accept operations: {0}")]
    CommunityDissolved(String),

    // Membership
    #[error("member not found: {0}")]
    MemberNotFound(String),

    #[error("already a member: {0}")]
    AlreadyMember(String),

    #[error("membership application not found: {0}")]
    ApplicationNotFound(String),

    #[error("insufficient role: required {required}, have {actual}")]
    InsufficientRole { required: String, actual: String },

    // Charter
    #[error("charter not found: {0}")]
    CharterNotFound(String),

    #[error("charter requires covenant alignment")]
    CovenantAlignmentRequired,

    #[error("charter already signed by: {0}")]
    CharterAlreadySigned(String),

    #[error("minimum founders not met: required {required}, have {actual}")]
    MinimumFoundersNotMet { required: usize, actual: usize },

    // Proposal
    #[error("proposal not found: {0}")]
    ProposalNotFound(String),

    #[error("proposal is not in {expected} status, currently {actual}")]
    InvalidProposalStatus { expected: String, actual: String },

    #[error("proposal cannot be edited in current status")]
    ProposalNotEditable,

    #[error("voting is not open for this proposal")]
    VotingNotOpen,

    #[error("voting period has expired")]
    VotingExpired,

    // Vote
    #[error("already voted on this proposal: {0}")]
    AlreadyVoted(String),

    #[error("not eligible to vote: {0}")]
    NotEligibleToVote(String),

    #[error("quorum not met: required {required:.2}, got {actual:.2}")]
    QuorumNotMet { required: f64, actual: f64 },

    // Decision
    #[error("unknown decision process: {0}")]
    UnknownDecisionProcess(String),

    // Mandate & Delegation
    #[error("mandate not found: {0}")]
    MandateNotFound(String),

    #[error("mandate has expired: {0}")]
    MandateExpired(String),

    #[error("delegate not found: {0}")]
    DelegateNotFound(String),

    #[error("delegate has been recalled: {0}")]
    DelegateRecalled(String),

    #[error("decision type not authorized by mandate: {0}")]
    DecisionNotAuthorized(String),

    #[error("circular delegation detected: {0}")]
    CircularDelegation(String),

    #[error("cannot delegate to self")]
    SelfDelegation,

    // Advisor Delegation
    #[error("advisor delegation not allowed by community policy")]
    AdvisorDelegationNotAllowed,

    #[error("proposal category requires human vote: {0}")]
    HumanVoteRequired(String),

    #[error("advisor auto-vote cap exceeded: {percentage:.1}% exceeds {cap:.1}% cap")]
    AdvisorCapExceeded { percentage: f64, cap: f64 },

    #[error("deliberation window has expired for proposal: {0}")]
    DeliberationWindowExpired(String),

    #[error("deliberation window not yet expired for proposal: {0}")]
    DeliberationWindowActive(String),

    #[error("member already has advisor delegation: {0}")]
    AdvisorDelegationExists(String),

    #[error("no advisor delegation found for member: {0}")]
    AdvisorDelegationNotFound(String),

    // Federation
    #[error("consortium not found: {0}")]
    ConsortiumNotFound(String),

    #[error("subsidiarity violation: {0}")]
    SubsidiarityViolation(String),

    #[error("community is not a consortium member: {0}")]
    NotConsortiumMember(String),

    #[error("federation agreement not found: {0}")]
    FederationNotFound(String),

    #[error("federation already exists between {community_a} and {community_b}")]
    FederationAlreadyExists {
        community_a: String,
        community_b: String,
    },

    // Assembly
    #[error("assembly not found: {0}")]
    AssemblyNotFound(String),

    #[error("assembly has concluded")]
    AssemblyConcluded,

    // Challenge
    #[error("challenge not found: {0}")]
    ChallengeNotFound(String),

    #[error("challenge brought in bad faith: {0}")]
    BadFaithChallenge(String),

    // Adjudication
    #[error("dispute not found: {0}")]
    DisputeNotFound(String),

    #[error("invalid dispute transition: {current} -> {target}")]
    InvalidDisputeTransition { current: String, target: String },

    #[error("adjudicator not found: {0}")]
    AdjudicatorNotFound(String),

    #[error("adjudicator not available: {0}")]
    AdjudicatorNotAvailable(String),

    #[error("resolution not found: {0}")]
    ResolutionNotFound(String),

    #[error("appeal not found: {0}")]
    AppealNotFound(String),

    #[error("appeal deadline has passed")]
    AppealDeadlinePassed,

    #[error("compliance not verified: {0}")]
    ComplianceNotVerified(String),

    // Union
    #[error("union not found: {0}")]
    UnionNotFound(String),

    #[error("union is dissolved: {0}")]
    UnionDissolved(String),

    #[error("unanimous consent required for union formation")]
    UnanimousConsentRequired,

    #[error("consent not provided by: {0}")]
    ConsentNotProvided(String),

    // Affected Party (R3B)
    #[error("affected party blocked proposal: group '{group}', {blocker_count} blocker(s)")]
    AffectedPartyBlocked { group: String, blocker_count: usize },

    #[error("affected party has not voted: {0}")]
    AffectedPartyNotVoted(String),

    #[error("deliberation minimum not met: required {required_secs}s, elapsed {elapsed_secs}s")]
    DeliberationMinimumNotMet {
        required_secs: u64,
        elapsed_secs: u64,
    },

    // Exit (R3C)
    #[error("exit penalty violates Sovereignty: {0}")]
    ExitPenaltyViolation(String),

    // Governance Health (R3D)
    #[error("governance budget full for community {community_id}: max {max} active proposals")]
    GovernanceBudgetFull { community_id: String, max: usize },

    #[error("member {member} has reached max terms for role {role}: {max} terms")]
    MaxTermsReached {
        member: String,
        role: String,
        max: usize,
    },

    #[error("member {member} is in cooling-off period for role {role} until {until}")]
    RoleCoolingOff {
        member: String,
        role: String,
        until: String,
    },

    // General
    #[error("invalid state transition: {current} -> {target}")]
    InvalidTransition { current: String, target: String },

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for KingdomError {
    fn from(e: serde_json::Error) -> Self {
        KingdomError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = KingdomError::CommunityNotFound("village_alpha".into());
        assert!(err.to_string().contains("village_alpha"));

        let err = KingdomError::MinimumFoundersNotMet {
            required: 3,
            actual: 1,
        };
        assert!(err.to_string().contains("3"));
        assert!(err.to_string().contains("1"));

        let err = KingdomError::CircularDelegation("alice -> bob -> alice".into());
        assert!(err.to_string().contains("circular"));

        let err = KingdomError::SubsidiarityViolation(
            "continental body deciding local matter".into(),
        );
        assert!(err.to_string().contains("subsidiarity"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<KingdomError>();
    }

    #[test]
    fn error_equality() {
        let a = KingdomError::CommunityNotFound("x".into());
        let b = KingdomError::CommunityNotFound("x".into());
        let c = KingdomError::CommunityNotFound("y".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn serialization_error_conversion() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let kingdom_err: KingdomError = json_err.into();
        assert!(matches!(kingdom_err, KingdomError::Serialization(_)));
    }
}
