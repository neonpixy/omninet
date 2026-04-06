//! # Polity — The Constitutional Guard
//!
//! The Covenant made executable. Polity maintains the rights, duties, and protections
//! of the Covenant as queryable, enforceable data structures. It performs constitutional
//! review on actions, detects breaches, manages the amendment process, tracks enactments,
//! and validates consent.
//!
//! Polity doesn't govern — it guards. It is the immune system that keeps everything else honest.
//!
//! ## The Three Registries
//!
//! - **Rights**: Entitlements recognized (not granted) by the Covenant. Portable and inalienable.
//! - **Duties**: Binding obligations arising from consciousness and kinship.
//! - **Protections**: Active prohibitions against specific forms of harm.
//!
//! ## Immutable Foundations
//!
//! The Core and Commons principles are hardcoded. No amendment process can touch them.
//! They are compile-time constants, not configurable values.
//!
//! ## Covenant Alignment
//!
//! **Dignity** — every right, duty, and protection traces back to irreducible worth.
//! **Sovereignty** — enactment is voluntary, withdrawal is always available.
//! **Consent** — continuous, informed, revocable. Coerced consent is void.

pub mod amendment;
pub mod breach;
pub mod consent;
pub mod constitutional_layers;
pub mod covenant_code;
pub mod duties;
pub mod enactment;
pub mod error;
pub mod immutable;
pub mod protections;
pub mod review;
pub mod rights;
pub mod weaponization;

// Re-exports for convenience.
pub use amendment::{Amendment, AmendmentStatus, AmendmentTrigger, ProposedChange};
pub use breach::{Breach, BreachRegistry, BreachSeverity, BreachStatus, ViolationType};
pub use consent::{
    ConsentRecord, ConsentRegistry, ConsentScope, ConsentValidation, ConsentValidator,
};
pub use duties::{BindingLevel, Duty, DutiesRegistry, DutyCategory, DutyScope};
pub use enactment::{
    Enactment, EnactmentRegistry, EnactmentStatus, EnactorType, Witness, DEFAULT_OATH,
};
pub use error::PolityError;
pub use immutable::ImmutableFoundation;
pub use protections::{
    ActionDescription, ProhibitionType, Protection, ProtectionsRegistry,
};
pub use review::{
    ConsentRequirement, ConstitutionalReview, ConstitutionalReviewer, ReviewResult,
    ReviewViolation,
};
pub use rights::{Right, RightCategory, RightScope, RightsRegistry};
pub use constitutional_layers::{
    AmendmentThreshold, AxiomAlignment, ClauseAmendment, ClauseRegistry, ConstitutionalClause,
    CovenantPart, CovenantPrecedent, PrecedentRegistry, PrecedentSearch, PrincipleReference,
    RatificationRecord, ReconstitutionGuard, ReconstitutionProposal, ReconstitutionStatus,
    ReconstitutionThreshold, ReconstitutionTrigger, AXIOMS,
};
pub use weaponization::{
    HarmClaim, InvocationCheck, InvocationConstraint, InvocationContext, InvocationResult,
    RightInvocation, WeaponizationReason,
};
pub use covenant_code::{
    // Part 00
    COVENANT_AXIOMS, PREAMBLE_DECLARATION,
    // Part 01
    NoDiscriminationBasis, SurveillanceProhibition, ExitRight, BreachCondition,
    // Part 02
    ResourceRelation, RegenerationObligation, AccessEquity, KnowledgeCommons,
    // Part 03
    ProtectedCharacteristic, AccommodationRequest, HistoricalHarmRecord, WhistleblowerProtection,
    // Part 04
    BeingProtection, PersonhoodPresumption, UnionConsent, LaborProtection, WealthCap,
    // Part 05
    CharterAlignment, CollectiveOwnership, TransparencyMandate, SunsetProvision, WorkerStakeholder,
    // Part 06
    SubsidiarityPrinciple, MandateDelegation, EmergencySunset, InviolableEmergencyRight,
    GraduatedEnforcement,
    // Part 07
    ConvocationRight, LawfulPurpose, AccessibilityMandate, LivingRecord,
    // Part 08
    DormancyDefault, CustodianAccountability, PublicOverride,
    // Part 09
    CompactAlignment, CompactRevocability, CompactTransparency,
    // Validator
    CovenantAction, CovenantCheck, CovenantValidation, CovenantValidator, BreachDetail,
};
