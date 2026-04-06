//! # Kingdom — Governance Primitives
//!
//! Toolkit for self-governance. Communities, charters, proposals, deliberation,
//! pluggable voting, mandate-based delegation, federation, dispute resolution.
//!
//! From Constellation Art. 1 §1: "All lawful governance under this Covenant
//! shall arise from communities."
//!
//! Kingdom doesn't govern — it provides the atoms from which governance is composed.
//! Communities choose their own decision processes, membership rules, federation
//! structures, and dispute resolution methods. Kingdom makes those choices expressible,
//! composable, and accountable.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — every governance structure protects the irreducible worth of every person.
//! **Sovereignty** — communities are self-governing; federation is voluntary; mandates are recallable.
//! **Consent** — governance imposed without active consent has no standing.

pub mod adjudication;
pub mod advisor_delegation;
pub mod affected_party;
pub mod ai_pool;
pub mod assembly;
pub mod challenge;
pub mod charter;
pub mod community;
pub mod covenant_court;
pub mod decision;
pub mod diplomacy;
pub mod error;
pub mod exit;
pub mod federation;
pub mod federation_agreement;
pub mod governance_health;
pub mod mandate;
pub mod membership;
pub mod proposal;
pub mod union;
pub mod vote;

// Re-exports for convenience.
pub use advisor_delegation::{
    AdvisorDelegation, AdvisorDelegationConfig, AdvisorDelegationRegistry, DelegateType,
    DelegationOverride, DelegationStats, DeliberationWindow, GovernanceAIPolicy,
};
pub use adjudication::{
    Adjudicator, AdjudicatorAssignment, AdjudicatorAvailability, AdjudicatorJurisdiction,
    AdjudicatorRecord, AdjudicatorRole, AdjudicatorStatus, Appeal, AppealDecision, AppealGround,
    AppealOutcome, AppealStatus, ComplianceRecord, ComplianceStatus, Counterclaim,
    DecisionOutcome, Dispute, DisputeContext, DisputeResponse, DisputeStatus, DisputeType,
    EvidenceItem, EvidenceType, Finding, FindingConfidence, HearingFormat, HearingParticipant,
    HearingRecord, OrderedRemedy, ParticipantRole, Qualification, QualificationType,
    RemedyAction, Resolution,
};
pub use assembly::{
    Assembly, AssemblyRecord, AssemblyStatus, AssemblyType, ConvocationTrigger, RecordType,
};
pub use challenge::{
    Challenge, ChallengeResponse, ChallengeStatus, ChallengeTarget, ChallengeType,
    ResponsePosition,
};
pub use charter::{
    AdjudicationFormat, AssetDistribution, Charter, CharterSignature, CovenantAlignment,
    DisputeResolutionConfig, DissolutionTerms, GovernanceStructure, LeadershipSelection,
    MediatorSelection, MembershipRules,
};
pub use community::{
    Community, CommunityBasis, CommunityMember, CommunityRole, CommunityStatus,
};
pub use decision::{
    ConsentProcess, ConsensusProcess, DecisionProcess, DelegationScope, DirectVoteProcess,
    LiquidDemocracyProcess, ProposalResult, RankedBallot, RankedChoiceProcess,
    SuperMajorityProcess, VoteDelegation,
};
pub use error::KingdomError;
pub use federation::{
    Consortium, ConsortiumCharter, ConsortiumGovernance, ConsortiumMember, ConsortiumStatus,
    GovernanceLevel, SubsidiarityCheck, VotingModel,
};
pub use federation_agreement::{
    FederationAgreement, FederationRegistry, FederationScope, FederationStatus,
};
pub use mandate::{
    AppointmentSource, Delegate, DelegateActivity, DelegateActivityType, DelegateRecall,
    Mandate, MandateDecision, RecallSignature, RecallStatus,
};
pub use membership::{
    ApplicationStatus, FormationRequirements, JoinProcess, MembershipApplication,
};
pub use proposal::{
    DecidingBody, DiscussionPost, Proposal, ProposalOutcome, ProposalStatus, ProposalType,
};
pub use union::{
    CeremonyRecord, ConsentSignature, DissolutionRecord, Union, UnionCharter, UnionFormation,
    UnionMember, UnionStatus, UnionType,
};
pub use covenant_court::{
    AdjudicatorPool, AdjudicatorSelection, CourtAdjudicator, CourtCase, CourtCaseStatus,
    CourtDecision, CourtDissent, CourtJurisdiction, CourtParty, CourtSubmission, CovenantCourt,
    PartyRole,
};
pub use vote::{
    ConsultationResult, DelegateVoteInfo, QuorumRequirement, Vote, VotePosition, VoteTally,
};
pub use affected_party::{
    AffectedPartyTag, AffectedPartyVote, MediationRecord, MediationStatus, ProposalConstraint,
    evaluate_affected_party_constraints, check_deliberation_minimum,
};
pub use exit::{
    ExitCost, ExitCostCalculator, ExitCostType, ExitPackage, ExitRetained, ExitTransferred,
    VisibleBond, reject_exit_penalty_clause,
};
pub use governance_health::{
    GovernanceBudget, ProposalQueue, QueuedProposal, RoleRotationPolicy, RoleTermTracker,
};
pub use diplomacy::{
    ChannelStatus, ChannelType, DiplomaticChannel, DiplomaticMessage, Liaison, LiaisonRole,
    ObligationType, Treaty, TreatyRatification, TreatyStatus, TreatyTerm,
};
pub use ai_pool::{
    AIPool, AIPoolPolicy, AIPoolReward, AIPoolUsage, MinimumCapabilities, PoolAccess,
    PoolPriority, PoolRequest, PoolResponse, PooledProvider, ProviderCapacity, RequestPriority,
};
