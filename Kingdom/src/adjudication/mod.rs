//! # Adjudication — Restorative Dispute Resolution
//!
//! The full dispute resolution pipeline: filing, response, hearing, resolution,
//! appeal, and compliance verification. Restorative, not punitive.
//!
//! From Constellation Art. 7 SS3: "Graduated Response Protocol — enforcement shall
//! proceed through graduated response, escalating only when lesser measures prove
//! insufficient."

pub mod adjudicator;
pub mod appeal;
pub mod compliance;
pub mod dispute;
pub mod hearing;
pub mod resolution;

pub use adjudicator::{
    Adjudicator, AdjudicatorAssignment, AdjudicatorAvailability, AdjudicatorJurisdiction,
    AdjudicatorRecord, AdjudicatorRole, AdjudicatorStatus, Qualification, QualificationType,
};
pub use appeal::{Appeal, AppealDecision, AppealGround, AppealOutcome, AppealStatus};
pub use compliance::{ComplianceRecord, ComplianceStatus};
pub use dispute::{
    Counterclaim, Dispute, DisputeContext, DisputeResponse, DisputeStatus, DisputeType,
    EvidenceItem, EvidenceType, ResponsePosition as DisputeResponsePosition,
};
pub use hearing::{HearingFormat, HearingParticipant, HearingRecord, ParticipantRole};
pub use resolution::{
    DecisionOutcome, Finding, FindingConfidence, OrderedRemedy, RemedyAction, Resolution,
};
