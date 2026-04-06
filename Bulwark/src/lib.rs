//! # Bulwark — Safety & Protection
//!
//! Care, not surveillance. Trust layers, health monitoring, reputation,
//! Kids Sphere insulation, consent validation.
//!
//! From Core Art. 5: "No structure of cruelty shall be lawful."
//!
//! Bulwark doesn't watch — it shields. Trust flows through people, not systems.
//! The village protects its children. Health is structural, never content.
//! Reputation is earned, never purchased.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — every person has inherent worth; trust layers protect, not exclude.
//! **Sovereignty** — you choose your verification path; no single method is mandated.
//! **Consent** — all monitoring is opt-in; parent oversight scales with age.

pub mod age_tier;
pub mod behavioral_drift;
pub mod child_safety;
pub mod consent;
pub mod error;
pub mod federation_scope;
pub mod health;
pub mod kids_exclusion;
pub mod kids_sphere;
pub mod network_origin;
pub mod permissions;
pub mod power_index;
pub mod reputation;
pub mod trust;
pub mod verification;

// Re-exports for convenience.
pub use age_tier::{AgeTier, AgeTierConfig};
pub use behavioral_drift::{
    Activity, BaselineMetrics, BehavioralBaseline, BehavioralDrift, DriftAlert, DriftAlertLevel,
    DriftComputer, DriftConfig, DriftFactor,
};
pub use child_safety::{
    ChildSafetyConcern, ChildSafetyFlag, ChildSafetyProtocol, ChildSafetyStatus,
    RealWorldResources, SilentRestriction,
};
pub use consent::{ConsentRecord, ConsentScope, ConsentValidator};
pub use error::BulwarkError;
pub use federation_scope::FederationScope;
pub use health::{
    CollectiveContentHealth, CollectiveCommunicationPattern, CollectiveHealthFactors,
    CollectiveHealthPulse, CollectiveHealthStatus, CrossMembershipLevel,
    EngagementDistribution, HealthSeverity, PowerDistribution, UserHealthStatus,
    ActivityPattern, CommunicationBalance, ConnectionLevel, ContentSentiment,
    UserHealthFactors, UserHealthPulse,
};
pub use kids_exclusion::{
    check_kids_sphere_access, ApprovalConfidence, KidsAdjudicationRecord, KidsExclusionBasis,
    KidsReviewSchedule, KidsReviewType, KidsSphereAccessResult, KidsSphereApproval,
    KidsSphereApprovalPolicy, KidsSphereExclusion, KidsSphereExclusionRegistry,
    KidsSphereExclusionScope, ParentalApproval,
};
pub use kids_sphere::{
    AllowedContactType, AllowedContentType, FamilyBond, KidConnectionRequest,
    KidConnectionRules, KidConnectionStatus, KidsSphereConfig, MinorDetectionReason,
    MinorRegistrationState, ParentLink, ParentOversight, SiloedMinor,
};
pub use network_origin::{BootstrapCapabilities, BootstrapPhase, NetworkOrigin};
pub use permissions::{
    Action, ActorConditionalPermission, ActorContext, CollectiveRole, Condition, ConditionOp,
    ConditionalPermission, Delegation, DelegationStore, DenialReason, EffectivePermission,
    Permission, PermissionChecker, PermissionContext, PermissionDecision, PermissionSource,
    ResourceScope, Role, RoleRegistry,
};
pub use power_index::{
    PowerAlert, PowerConcentrationComputer, PowerConcentrationConfig, PowerConcentrationIndex,
    PowerFactors,
};
pub use reputation::{
    Consequence, ConsequenceDuration, ConsequenceType, FraudIndicator, FraudSeverity,
    Reputation, ReputationEvent, ReputationEventType, ReputationFactors, RiskRecommendation,
    RiskScore, Standing, SuspiciousPattern, SuspiciousPatternType,
};
pub use trust::{
    BondCapabilities, BondChange, BondDepth, EntryMethod, LayerCapabilities,
    LayerTransitionBlocker, LayerTransitionEvidence, LayerTransitionRequest,
    LayerTransitionRequirements, LayerTransitionStatus, ShieldedRequirements, SponsorRecord,
    TrustChain, TrustLayer, VerifiedRequirements, VisibleBond, VouchRecord,
    VouchedRequirements,
};
pub use verification::{
    MutualVouch, ProximityBond, ProximityMethod, ProximityProof, SponsorEligibility, Sponsorship,
    SponsorshipStatus, VerificationEvidence, VerificationMethod, VerificationResult,
    VouchDiversityCheck, VouchEligibility, VouchRule, VouchRules,
};
