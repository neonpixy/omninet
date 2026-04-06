//! # Jail — Accountability Primitives
//!
//! Web of trust and restorative accountability. Trust graph, pattern detection,
//! graduated response, community admission, duty to warn.
//!
//! From Constellation Art. 7 §1: "Enforcement of this Covenant shall arise through
//! the coordinated withdrawal of cooperation, consent, and recognition — never
//! through domination or violence."
//!
//! Jail doesn't punish — it holds accountable. The trust graph records who vouches
//! for whom. Flags surface concerns. Patterns emerge across communities. Responses
//! graduate from education to exclusion. And even exclusion has a path back — because
//! there are no permanent castes under the Covenant.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — the accused always keep their rights; response is proportional.
//! **Sovereignty** — communities decide their own accountability parameters.
//! **Consent** — flags require evidence; weaponization is detected and blocked.

pub mod admission;
pub mod appeal;
pub mod config;
pub mod error;
pub mod federation_scope;
pub mod flag;
pub mod response;
pub mod reverification;
pub mod rights;
pub mod sustained_exclusion;
pub mod trust_graph;

// Re-exports for convenience.
pub use admission::{check_admission, check_admission_scoped, AdmissionAction, AdmissionRecommendation};
pub use federation_scope::FederationScope;
pub use appeal::{Appeal, AppealDecision, AppealGround, AppealOutcome, AppealStatus};
pub use config::JailConfig;
pub use error::JailError;
pub use flag::{
    AccountabilityFlag, AbuseConsequence, AbuseIndicator, AbusePattern, CommunityAction,
    DutyToWarn, FlagCategory, FlagContext, FlagPattern, FlagReview, FlagReviewStatus,
    FlagSeverity, PatternSummary, ReviewOutcome, WarningRecord,
};
pub use response::{
    ExclusionDecision, ExclusionReview, GraduatedResponse, ProtectiveExclusion, Remedy,
    RemedyStatus, RemedyType, RestorationPath, RestorationProgress, ResponseLevel,
    ResponseRecord, ReviewSchedule,
};
pub use reverification::{
    AttestationRequirements, ReVerificationAttestation, ReVerificationReason,
    ReVerificationSession, ReVerificationState,
};
pub use rights::{AccusedRights, ReporterProtection};
pub use sustained_exclusion::{
    AdjudicatedSeverity, CommunityAffirmation, EvidenceSource, ExclusionEvidence, ExclusionScope,
    SustainedExclusion, SustainedExclusionBasis, SustainedExclusionRequest, SustainedReview,
    SustainedReviewFinding, SustainedReviewSchedule, SustainedReviewType,
};
pub use trust_graph::{
    NetworkFlag, NetworkIntelligence, NetworkVerification, TrustGraph, TrustRecommendation,
    VerificationEdge, VerificationPattern, VerificationSentiment, query_flags_scoped,
    query_intelligence_scoped,
};
