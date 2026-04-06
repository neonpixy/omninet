//! # Covenant Encoding (R1E)
//!
//! Encodes ~70% of the Covenant's mechanical rules from all 10 Parts as type constraints
//! and validation functions. The remaining ~30% that requires human judgment returns
//! [`CovenantValidation::RequiresHumanJudgment`] for Star Court evaluation.
//!
//! ## Per-Part Encoding
//!
//! - **Part 00 (Preamble):** Constants and axiom declarations.
//! - **Part 01 (Core):** `NoDiscriminationBasis`, `SurveillanceProhibition`, `ExitRight`, `BreachCondition`.
//! - **Part 02 (Commons):** `ResourceRelation`, `RegenerationObligation`, `AccessEquity`, `KnowledgeCommons`.
//! - **Part 03 (Coexistence):** `ProtectedCharacteristic`, `AccommodationRequest`, `HistoricalHarmRecord`, `WhistleblowerProtection`.
//! - **Part 04 (Conjunction):** `BeingProtection`, `PersonhoodPresumption`, `UnionConsent`, `LaborProtection`, `WealthCap`.
//! - **Part 05 (Consortium):** `CharterAlignment`, `CollectiveOwnership`, `TransparencyMandate`, `SunsetProvision`, `WorkerStakeholder`.
//! - **Part 06 (Constellation):** `SubsidiarityPrinciple`, `MandateDelegation`, `EmergencySunset`, `InviolableEmergencyRights`, `GraduatedEnforcement`.
//! - **Part 07 (Convocation):** `ConvocationRight`, `LawfulPurpose`, `AccessibilityMandate`, `LivingRecord`.
//! - **Part 08 (Continuum):** `ReconstitutionTrigger` (reused from R1A), `DormancyDefault`, `CustodianAccountability`, `PublicOverride`.
//! - **Part 09 (Compact):** `CompactAlignment`, `CompactRevocability`, `CompactTransparency`.
//!
//! ## Unified Validation
//!
//! [`CovenantValidator`] provides a single entry point:
//! `validate_action(action) -> CovenantValidation` (Permitted | Breach | RequiresHumanJudgment).
//!
//! ## Integration
//!
//! `ConstitutionalReviewer::review()` delegates to `CovenantValidator::validate_action()`.
//! The reviewer becomes a thin wrapper around the comprehensive validation engine.

use serde::{Deserialize, Serialize};

use crate::breach::BreachSeverity;
use crate::constitutional_layers::CovenantPart;

// ===========================================================================
// Part 00: Preamble — Constants and Axiom Declarations
// ===========================================================================

/// The three axioms that underpin the entire Covenant. Symbolic constants.
pub const COVENANT_AXIOMS: [&str; 3] = ["Dignity", "Sovereignty", "Consent"];

/// The Preamble's declaration of purpose.
pub const PREAMBLE_DECLARATION: &str =
    "We the People, in recognition of our shared dignity and interdependence, \
     do ordain and establish this Covenant.";

// ===========================================================================
// Part 01: Core — Rights, Duties, Prohibitions
// ===========================================================================

/// Bases on which discrimination is prohibited. Exhaustive per Covenant Core Art. 5 Sec. 1.
///
/// Actions that discriminate on any of these bases are automatically a breach.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NoDiscriminationBasis {
    /// Race or ethnic heritage.
    Race,
    /// Gender identity or expression.
    Gender,
    /// Socioeconomic class.
    Class,
    /// Physical, cognitive, or sensory disability.
    Disability,
    /// Religious belief or practice.
    Religion,
    /// Cultural background or tradition.
    Culture,
    /// Language spoken or signed.
    Language,
    /// Sexual identity or orientation.
    SexualIdentity,
    /// Age, whether young or old.
    Age,
    /// Place of origin or nationality.
    Origin,
    /// Personal belief system.
    Belief,
    /// Body type or physical appearance.
    Body,
    /// Substrate: human vs. synthetic consciousness.
    Substrate,
}

impl NoDiscriminationBasis {
    /// All known discrimination bases.
    pub const ALL: &[NoDiscriminationBasis] = &[
        Self::Race,
        Self::Gender,
        Self::Class,
        Self::Disability,
        Self::Religion,
        Self::Culture,
        Self::Language,
        Self::SexualIdentity,
        Self::Age,
        Self::Origin,
        Self::Belief,
        Self::Body,
        Self::Substrate,
    ];
}

/// Type-level proof that surveillance is forbidden.
///
/// From Core Art. 5 Sec. 2: surveillance, behavioral manipulation, and unwarranted
/// intrusion are absolutely prohibited. This type has no constructors that return `true`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SurveillanceProhibition;

impl SurveillanceProhibition {
    /// Surveillance is never allowed. This function always returns `false`.
    #[must_use]
    pub const fn allowed() -> bool {
        false
    }

    /// Validate that an action does not constitute surveillance.
    pub fn validate(involves_surveillance: bool) -> Result<(), BreachDetail> {
        if involves_surveillance {
            Err(BreachDetail {
                part: CovenantPart::Core,
                article: "Core Art. 5 Sec. 2".into(),
                violation: "Surveillance is absolutely prohibited".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// All governance structures must implement exit.
///
/// From Core Art. 2 Sec. 6: the right to refuse and resist.
/// Any governance structure that does not offer `revoke_consent()` and `exit()` is a breach.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExitRight {
    /// Whether the structure offers consent revocation.
    pub has_revoke_consent: bool,
    /// Whether the structure offers an exit path.
    pub has_exit: bool,
}

impl ExitRight {
    /// Validate that a governance structure honors the exit right.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if !self.has_revoke_consent || !self.has_exit {
            Err(BreachDetail {
                part: CovenantPart::Core,
                article: "Core Art. 2 Sec. 6".into(),
                violation: format!(
                    "Governance structure missing: {}{}",
                    if !self.has_revoke_consent {
                        "revoke_consent "
                    } else {
                        ""
                    },
                    if !self.has_exit { "exit" } else { "" },
                ),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// Conditions that constitute a breach.
///
/// From Core Art. 8 Sec. 2: a breach occurs when any of these conditions are met.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreachCondition {
    pub violates_dignity: bool,
    pub violates_duty: bool,
    pub refuses_correction: bool,
    pub perpetuates_domination: bool,
}

impl BreachCondition {
    /// Whether any breach condition is active.
    pub fn is_breach(&self) -> bool {
        self.violates_dignity
            || self.violates_duty
            || self.refuses_correction
            || self.perpetuates_domination
    }

    /// Validate and return a breach detail if any condition is met.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.is_breach() {
            let mut violations = Vec::new();
            if self.violates_dignity {
                violations.push("violates dignity");
            }
            if self.violates_duty {
                violations.push("violates duty");
            }
            if self.refuses_correction {
                violations.push("refuses correction");
            }
            if self.perpetuates_domination {
                violations.push("perpetuates domination");
            }
            Err(BreachDetail {
                part: CovenantPart::Core,
                article: "Core Art. 8 Sec. 2".into(),
                violation: format!("Breach conditions: {}", violations.join(", ")),
                severity: if self.violates_dignity || self.perpetuates_domination {
                    BreachSeverity::Grave
                } else {
                    BreachSeverity::Significant
                },
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 02: Commons — Resource Stewardship Constraints
// ===========================================================================

/// The only lawful relation to shared resources. There is no `Ownership` variant.
///
/// From Commons Art. 1: all shared resources are held in stewardship, never owned.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResourceRelation {
    /// The only lawful relation. No `Ownership` variant exists.
    Stewardship,
}

impl ResourceRelation {
    /// Validate that a claimed resource relation is lawful (Stewardship only).
    pub fn validate_not_ownership(claimed: &str) -> Result<(), BreachDetail> {
        let lower = claimed.to_lowercase();
        if lower.contains("ownership") || lower.contains("private property") {
            Err(BreachDetail {
                part: CovenantPart::Commons,
                article: "Commons Art. 1".into(),
                violation: "Resources may only be held in Stewardship, never Ownership".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// Resources returned must equal or exceed resources taken.
///
/// From Commons Art. 2: regeneration is an obligation, not an aspiration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegenerationObligation {
    pub resources_taken: f64,
    pub resources_returned: f64,
    pub period_description: String,
}

impl RegenerationObligation {
    /// Validate that regeneration obligations are met.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.resources_returned < self.resources_taken {
            Err(BreachDetail {
                part: CovenantPart::Commons,
                article: "Commons Art. 2".into(),
                violation: format!(
                    "Regeneration deficit: returned {:.2} < taken {:.2} in period '{}'",
                    self.resources_returned, self.resources_taken, self.period_description
                ),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// No access denied based on economic status.
///
/// From Commons Art. 3: equitable access is a right.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessEquity {
    pub access_denied: bool,
    pub denial_based_on_economic_status: bool,
}

impl AccessEquity {
    /// Validate equitable access.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.access_denied && self.denial_based_on_economic_status {
            Err(BreachDetail {
                part: CovenantPart::Commons,
                article: "Commons Art. 3".into(),
                violation: "Access denied based on economic status".into(),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// Innovations are open access by default.
///
/// From Commons Art. 4: knowledge belongs to the commons.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KnowledgeCommons {
    /// The default and only lawful state for innovations.
    OpenAccess,
}

impl KnowledgeCommons {
    /// Validate that knowledge is not enclosed.
    pub fn validate_not_enclosed(is_enclosed: bool) -> Result<(), BreachDetail> {
        if is_enclosed {
            Err(BreachDetail {
                part: CovenantPart::Commons,
                article: "Commons Art. 4".into(),
                violation: "Knowledge must be OpenAccess by default; enclosure is prohibited".into(),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 03: Coexistence — Anti-Discrimination and Cultural Protection
// ===========================================================================

/// Protected characteristics — exhaustive list from the Covenant.
///
/// From Coexistence Art. 1: discrimination on any of these is prohibited.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProtectedCharacteristic {
    /// Race.
    Race,
    /// Ethnicity or ethnic heritage.
    Ethnicity,
    /// Gender identity or expression.
    Gender,
    /// Sexual orientation.
    SexualOrientation,
    /// Physical, cognitive, or sensory disability.
    Disability,
    /// Age, whether young or old.
    Age,
    /// Religious belief or practice.
    Religion,
    /// Language spoken or signed.
    Language,
    /// Cultural background or tradition.
    Culture,
    /// National origin or place of birth.
    NationalOrigin,
    /// Socioeconomic status or wealth level.
    SocioeconomicStatus,
    /// Body type or physical appearance.
    BodyType,
    /// Neurological difference (e.g., autism, ADHD, dyslexia).
    NeurologicalDifference,
    /// Substrate: human vs. synthetic consciousness.
    Substrate,
}

impl ProtectedCharacteristic {
    /// All known protected characteristics.
    pub const ALL: &[ProtectedCharacteristic] = &[
        Self::Race,
        Self::Ethnicity,
        Self::Gender,
        Self::SexualOrientation,
        Self::Disability,
        Self::Age,
        Self::Religion,
        Self::Language,
        Self::Culture,
        Self::NationalOrigin,
        Self::SocioeconomicStatus,
        Self::BodyType,
        Self::NeurologicalDifference,
        Self::Substrate,
    ];
}

/// An accommodation request with bounds.
///
/// From Coexistence Art. 2: reasonable accommodation is required,
/// but stops where it would cause harm to others.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccommodationRequest {
    pub characteristic: ProtectedCharacteristic,
    pub description: String,
    pub harms_others: bool,
}

impl AccommodationRequest {
    /// Validate an accommodation request.
    ///
    /// An accommodation must be provided unless it harms others.
    /// When it harms others, this requires human judgment.
    pub fn validate(&self) -> CovenantValidation {
        if self.harms_others {
            CovenantValidation::RequiresHumanJudgment(format!(
                "Accommodation for {:?} may harm others: '{}'. Star Court must weigh competing needs.",
                self.characteristic, self.description
            ))
        } else {
            CovenantValidation::Permitted
        }
    }
}

/// An immutable archive of documented injustices.
///
/// From Coexistence Art. 3: historical harms are recorded and never erased.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoricalHarmRecord {
    pub description: String,
    pub affected_groups: Vec<String>,
    pub source: String,
    pub is_immutable: bool,
}

impl HistoricalHarmRecord {
    /// Create a new immutable harm record.
    pub fn new(
        description: impl Into<String>,
        affected_groups: Vec<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            description: description.into(),
            affected_groups,
            source: source.into(),
            is_immutable: true,
        }
    }

    /// Validate that a harm record has not been tampered with.
    pub fn validate_immutability(&self) -> Result<(), BreachDetail> {
        if !self.is_immutable {
            Err(BreachDetail {
                part: CovenantPart::Coexistence,
                article: "Coexistence Art. 3".into(),
                violation: "Historical harm records must be immutable".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// Retaliation against whistleblowers is always a breach.
///
/// From Coexistence Art. 4: structural protection for those who report harm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhistleblowerProtection;

impl WhistleblowerProtection {
    /// Validate that no retaliation has occurred against a reporter.
    pub fn validate(retaliation_against_reporter: bool) -> Result<(), BreachDetail> {
        if retaliation_against_reporter {
            Err(BreachDetail {
                part: CovenantPart::Coexistence,
                article: "Coexistence Art. 4".into(),
                violation: "Retaliation against reporters/whistleblowers is always a breach".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 04: Conjunction — Being, Kinship, Labor
// ===========================================================================

/// All beings have legal protection regardless of intelligence.
///
/// From Conjunction Art. 1: no intelligence threshold for protection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BeingProtection;

impl BeingProtection {
    /// Validate that protection is not conditional on intelligence level.
    pub fn validate(protection_conditional_on_intelligence: bool) -> Result<(), BreachDetail> {
        if protection_conditional_on_intelligence {
            Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 1".into(),
                violation: "Protection must not be conditional on intelligence level".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// When doubt exists about consciousness, presume personhood.
///
/// From Conjunction Art. 2: the precautionary principle applied to consciousness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonhoodPresumption;

impl PersonhoodPresumption {
    /// Validate that consciousness doubt is resolved in favor of personhood.
    pub fn validate(
        consciousness_uncertain: bool,
        personhood_denied: bool,
    ) -> Result<(), BreachDetail> {
        if consciousness_uncertain && personhood_denied {
            Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 2".into(),
                violation: "When consciousness is uncertain, personhood must be presumed".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// All unions require continuous, informed, revocable consent.
///
/// From Conjunction Art. 4: consent-based personal unions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnionConsent {
    pub is_continuous: bool,
    pub is_informed: bool,
    pub is_revocable: bool,
}

impl UnionConsent {
    /// Validate that union consent meets all requirements.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut missing = Vec::new();
        if !self.is_continuous {
            missing.push("continuous");
        }
        if !self.is_informed {
            missing.push("informed");
        }
        if !self.is_revocable {
            missing.push("revocable");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 4".into(),
                violation: format!("Union consent must be: {}", missing.join(", ")),
                severity: BreachSeverity::Significant,
            })
        }
    }
}

/// Livelihood as birthright; no work required for survival.
///
/// From Conjunction Art. 7: UBI is a right. Coerced labor is prohibited.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaborProtection {
    pub work_required_for_survival: bool,
    pub labor_coerced: bool,
}

impl LaborProtection {
    /// Validate labor protections.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.work_required_for_survival {
            return Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 7 Sec. 2".into(),
                violation: "No person shall be made to work to survive (UBI is a right)".into(),
                severity: BreachSeverity::Grave,
            });
        }
        if self.labor_coerced {
            return Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 7 Sec. 3".into(),
                violation: "Coerced labor is prohibited".into(),
                severity: BreachSeverity::Grave,
            });
        }
        Ok(())
    }
}

/// Accumulation limits enforced by FlowBack.
///
/// From Conjunction Art. 8: wealth caps prevent domination through accumulation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WealthCap {
    pub current_wealth: f64,
    pub cap: f64,
}

impl WealthCap {
    /// Validate that wealth does not exceed the cap.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.current_wealth > self.cap {
            Err(BreachDetail {
                part: CovenantPart::Conjunction,
                article: "Conjunction Art. 8".into(),
                violation: format!(
                    "Wealth ({:.2}) exceeds cap ({:.2}); FlowBack must be applied",
                    self.current_wealth, self.cap
                ),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 05: Consortium — Enterprise Constraints
// ===========================================================================

/// Charter must affirm Core AND Commons.
///
/// From Consortium Art. 1: every enterprise charter must align with the Covenant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharterAlignment {
    pub affirms_core: bool,
    pub affirms_commons: bool,
}

impl CharterAlignment {
    /// Validate that a charter affirms both Core and Commons.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if !self.affirms_core || !self.affirms_commons {
            Err(BreachDetail {
                part: CovenantPart::Consortium,
                article: "Consortium Art. 1".into(),
                violation: format!(
                    "Charter must affirm both Core and Commons (core: {}, commons: {})",
                    self.affirms_core, self.affirms_commons
                ),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// Enterprise ownership is always Collective, never Private.
///
/// From Consortium Art. 2: no private ownership of enterprises.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CollectiveOwnership {
    /// The only lawful ownership type. No `Private` variant exists.
    Collective,
}

impl CollectiveOwnership {
    /// Validate that ownership is collective.
    pub fn validate_not_private(is_private: bool) -> Result<(), BreachDetail> {
        if is_private {
            Err(BreachDetail {
                part: CovenantPart::Consortium,
                article: "Consortium Art. 2".into(),
                violation: "Enterprise ownership must be Collective, never Private".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// All income, spending, and partnerships must be public and auditable.
///
/// From Consortium Art. 3: transparency is not optional.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransparencyMandate {
    pub income_public: bool,
    pub spending_public: bool,
    pub partnerships_public: bool,
}

impl TransparencyMandate {
    /// Validate that all transparency requirements are met.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut hidden = Vec::new();
        if !self.income_public {
            hidden.push("income");
        }
        if !self.spending_public {
            hidden.push("spending");
        }
        if !self.partnerships_public {
            hidden.push("partnerships");
        }
        if hidden.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Consortium,
                article: "Consortium Art. 3".into(),
                violation: format!(
                    "The following must be public and auditable: {}",
                    hidden.join(", ")
                ),
                severity: BreachSeverity::Significant,
            })
        }
    }
}

/// Charters expire in 10 years unless renewed.
///
/// From Consortium Art. 4: sunset prevents institutional ossification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SunsetProvision {
    /// Years since charter was last renewed.
    pub years_since_renewal: u32,
}

impl SunsetProvision {
    /// Maximum charter lifetime in years before renewal is required.
    pub const MAX_CHARTER_YEARS: u32 = 10;

    /// Validate that a charter has not expired.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.years_since_renewal > Self::MAX_CHARTER_YEARS {
            Err(BreachDetail {
                part: CovenantPart::Consortium,
                article: "Consortium Art. 4".into(),
                violation: format!(
                    "Charter expired: {} years since renewal (max {})",
                    self.years_since_renewal,
                    Self::MAX_CHARTER_YEARS
                ),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// All workers are stakeholders with voice in governance.
///
/// From Consortium Art. 5: no one works without a say.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerStakeholder {
    pub workers_have_governance_voice: bool,
}

impl WorkerStakeholder {
    /// Validate that workers are included as stakeholders.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if !self.workers_have_governance_voice {
            Err(BreachDetail {
                part: CovenantPart::Consortium,
                article: "Consortium Art. 5".into(),
                violation: "All workers must be stakeholders with voice in governance".into(),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 06: Constellation — Governance Constraints
// ===========================================================================

/// Decisions at the most local capable level.
///
/// From Constellation Art. 1: subsidiarity prevents overcentralization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubsidiarityPrinciple {
    /// Whether a more local body could handle this decision.
    pub more_local_body_capable: bool,
    /// Whether the decision is being made at a higher level.
    pub decided_at_higher_level: bool,
}

impl SubsidiarityPrinciple {
    /// Validate subsidiarity.
    pub fn validate(&self) -> CovenantValidation {
        if self.more_local_body_capable && self.decided_at_higher_level {
            // This often requires human judgment about "capability"
            CovenantValidation::RequiresHumanJudgment(
                "A more local body may be capable of this decision. \
                 Star Court should evaluate whether subsidiarity is violated."
                    .into(),
            )
        } else {
            CovenantValidation::Permitted
        }
    }
}

/// Delegates carry specific mandates; revocable and term-limited.
///
/// From Constellation Art. 2: delegation is not abdication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MandateDelegation {
    pub has_specific_mandate: bool,
    pub is_revocable: bool,
    pub is_term_limited: bool,
}

impl MandateDelegation {
    /// Validate that delegation meets all requirements.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut missing = Vec::new();
        if !self.has_specific_mandate {
            missing.push("specific mandate");
        }
        if !self.is_revocable {
            missing.push("revocability");
        }
        if !self.is_term_limited {
            missing.push("term limit");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Constellation,
                article: "Constellation Art. 2".into(),
                violation: format!("Delegation missing: {}", missing.join(", ")),
                severity: BreachSeverity::Significant,
            })
        }
    }
}

/// Emergency powers are hardcoded to expire. No vote extends them.
///
/// From Constellation Art. 3: MAX_INITIAL_DAYS=30, MAX_RENEWAL_DAYS=30, ABSOLUTE_MAX_DAYS=90.
/// These are constants. Not configurable. Not overridable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmergencySunset {
    /// Days since emergency was declared.
    pub days_elapsed: u32,
    /// Number of renewals granted.
    pub renewals: u32,
}

impl EmergencySunset {
    /// Maximum initial emergency period in days.
    pub const MAX_INITIAL_DAYS: u32 = 30;
    /// Maximum renewal period in days.
    pub const MAX_RENEWAL_DAYS: u32 = 30;
    /// Absolute maximum emergency duration in days, regardless of renewals.
    pub const ABSOLUTE_MAX_DAYS: u32 = 90;

    /// Validate that emergency powers have not exceeded their limits.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if self.days_elapsed > Self::ABSOLUTE_MAX_DAYS {
            return Err(BreachDetail {
                part: CovenantPart::Constellation,
                article: "Constellation Art. 3".into(),
                violation: format!(
                    "Emergency exceeded absolute maximum: {} days > {} days. \
                     No vote may extend this.",
                    self.days_elapsed,
                    Self::ABSOLUTE_MAX_DAYS
                ),
                severity: BreachSeverity::Existential,
            });
        }

        let max_for_renewals = Self::MAX_INITIAL_DAYS + (self.renewals * Self::MAX_RENEWAL_DAYS);
        let effective_max = max_for_renewals.min(Self::ABSOLUTE_MAX_DAYS);

        if self.days_elapsed > effective_max {
            return Err(BreachDetail {
                part: CovenantPart::Constellation,
                article: "Constellation Art. 3".into(),
                violation: format!(
                    "Emergency exceeded allowed period: {} days > {} days (initial {} + {} renewals * {})",
                    self.days_elapsed,
                    effective_max,
                    Self::MAX_INITIAL_DAYS,
                    self.renewals,
                    Self::MAX_RENEWAL_DAYS
                ),
                severity: BreachSeverity::Grave,
            });
        }

        Ok(())
    }
}

/// Rights that are NEVER suspended during emergency.
///
/// From Constellation Art. 3: dignity, consent, rights, and exclusion protection
/// are inviolable even in emergency.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InviolableEmergencyRight {
    /// Inherent worth cannot be suspended, even in emergency.
    Dignity,
    /// Consent requirements remain in force during emergency.
    Consent,
    /// Fundamental rights are not suspended during emergency.
    Rights,
    /// No one may be excluded or expelled during emergency.
    ExclusionProtection,
}

impl InviolableEmergencyRight {
    /// All inviolable emergency rights.
    pub const ALL: &[InviolableEmergencyRight] = &[
        Self::Dignity,
        Self::Consent,
        Self::Rights,
        Self::ExclusionProtection,
    ];

    /// Validate that an emergency action does not suspend inviolable rights.
    pub fn validate_not_suspended(suspended_rights: &[InviolableEmergencyRight]) -> Result<(), BreachDetail> {
        if suspended_rights.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Constellation,
                article: "Constellation Art. 3".into(),
                violation: format!(
                    "Inviolable rights suspended during emergency: {:?}. These may NEVER be suspended.",
                    suspended_rights
                ),
                severity: BreachSeverity::Existential,
            })
        }
    }
}

/// Graduated enforcement: education -> censure -> economic disengagement -> isolation.
///
/// From Constellation Art. 4: matches Jail's enforcement ladder.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GraduatedEnforcement {
    /// Step 1: Education and dialogue.
    Education,
    /// Step 2: Formal censure.
    Censure,
    /// Step 3: Economic disengagement.
    EconomicDisengagement,
    /// Step 4: Isolation (last resort).
    Isolation,
}

impl GraduatedEnforcement {
    /// Numeric rank of this enforcement step (for ordering validation).
    fn rank(self) -> u8 {
        match self {
            Self::Education => 0,
            Self::Censure => 1,
            Self::EconomicDisengagement => 2,
            Self::Isolation => 3,
        }
    }

    /// Validate that enforcement follows the graduated order.
    ///
    /// A higher step cannot be applied without first attempting lower steps.
    pub fn validate(proposed: GraduatedEnforcement, highest_completed: Option<GraduatedEnforcement>) -> Result<(), BreachDetail> {
        match highest_completed {
            None if proposed != GraduatedEnforcement::Education => {
                Err(BreachDetail {
                    part: CovenantPart::Constellation,
                    article: "Constellation Art. 4".into(),
                    violation: format!(
                        "Cannot apply {:?} without first completing Education",
                        proposed
                    ),
                    severity: BreachSeverity::Significant,
                })
            }
            Some(completed) if proposed.rank() > completed.rank() + 1 => {
                Err(BreachDetail {
                    part: CovenantPart::Constellation,
                    article: "Constellation Art. 4".into(),
                    violation: format!(
                        "Cannot skip from {:?} to {:?}; enforcement must be graduated",
                        completed, proposed
                    ),
                    severity: BreachSeverity::Significant,
                })
            }
            _ => Ok(()),
        }
    }
}

// ===========================================================================
// Part 07: Convocation — Assembly Rights
// ===========================================================================

/// The right to convene is inalienable.
///
/// From Convocation Art. 1: no prior permission, no retaliation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvocationRight;

impl ConvocationRight {
    /// Validate that no prior permission is required and no retaliation occurs.
    pub fn validate(
        prior_permission_required: bool,
        retaliation_for_assembly: bool,
    ) -> Result<(), BreachDetail> {
        if prior_permission_required {
            return Err(BreachDetail {
                part: CovenantPart::Convocation,
                article: "Convocation Art. 1".into(),
                violation: "Convocation requires no prior permission".into(),
                severity: BreachSeverity::Grave,
            });
        }
        if retaliation_for_assembly {
            return Err(BreachDetail {
                part: CovenantPart::Convocation,
                article: "Convocation Art. 1".into(),
                violation: "Retaliation for assembly is absolutely prohibited".into(),
                severity: BreachSeverity::Grave,
            });
        }
        Ok(())
    }
}

/// Assembly must have clear purpose, consent-based participation, Covenant alignment.
///
/// From Convocation Art. 2: assemblies are purposeful, not arbitrary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawfulPurpose {
    pub has_clear_purpose: bool,
    pub participation_consent_based: bool,
    pub covenant_aligned: bool,
}

impl LawfulPurpose {
    /// Validate that an assembly meets lawful purpose requirements.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut missing = Vec::new();
        if !self.has_clear_purpose {
            missing.push("clear purpose");
        }
        if !self.participation_consent_based {
            missing.push("consent-based participation");
        }
        if !self.covenant_aligned {
            missing.push("Covenant alignment");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Convocation,
                article: "Convocation Art. 2".into(),
                violation: format!("Assembly lacks: {}", missing.join(", ")),
                severity: BreachSeverity::Minor,
            })
        }
    }
}

/// Translation, mobility, childcare, hybrid participation required.
///
/// From Convocation Art. 3: barriers to participation are barriers to democracy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibilityMandate {
    pub translation_available: bool,
    pub mobility_accommodations: bool,
    pub childcare_available: bool,
    pub hybrid_participation: bool,
}

impl AccessibilityMandate {
    /// Validate that all accessibility requirements are met.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut missing = Vec::new();
        if !self.translation_available {
            missing.push("translation");
        }
        if !self.mobility_accommodations {
            missing.push("mobility accommodations");
        }
        if !self.childcare_available {
            missing.push("childcare");
        }
        if !self.hybrid_participation {
            missing.push("hybrid participation");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Convocation,
                article: "Convocation Art. 3".into(),
                violation: format!("Missing accessibility: {}", missing.join(", ")),
                severity: BreachSeverity::Significant,
            })
        }
    }
}

/// Assembly records: who attended, what proposed, what resolved — archived in Yoke.
///
/// From Convocation Art. 4: living records ensure accountability and memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivingRecord {
    pub attendees_recorded: bool,
    pub proposals_recorded: bool,
    pub resolutions_recorded: bool,
    pub archived_in_yoke: bool,
}

impl LivingRecord {
    /// Validate that assembly records are complete.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut missing = Vec::new();
        if !self.attendees_recorded {
            missing.push("attendees");
        }
        if !self.proposals_recorded {
            missing.push("proposals");
        }
        if !self.resolutions_recorded {
            missing.push("resolutions");
        }
        if !self.archived_in_yoke {
            missing.push("Yoke archive");
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Convocation,
                article: "Convocation Art. 4".into(),
                violation: format!("Living record incomplete: {}", missing.join(", ")),
                severity: BreachSeverity::Minor,
            })
        }
    }
}

// ===========================================================================
// Part 08: Continuum — Meta-Governance
// ===========================================================================

/// The Continuum sleeps unless invoked. Stability is the default.
///
/// From Continuum Art. 4: noninterference is the presumption.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DormancyDefault;

impl DormancyDefault {
    /// Validate that the Continuum is only invoked when triggered.
    pub fn validate(is_invoked: bool, has_trigger: bool) -> Result<(), BreachDetail> {
        if is_invoked && !has_trigger {
            Err(BreachDetail {
                part: CovenantPart::Continuum,
                article: "Continuum Art. 4".into(),
                violation:
                    "Continuum invoked without valid trigger; dormancy is the default state".into(),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// Deletion, obfuscation, or falsification of records revokes custodian standing.
///
/// From Continuum Art. 5: custodians are held to the highest standard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustodianAccountability {
    pub records_deleted: bool,
    pub records_obfuscated: bool,
    pub records_falsified: bool,
}

impl CustodianAccountability {
    /// Validate custodian accountability.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut violations = Vec::new();
        if self.records_deleted {
            violations.push("deletion");
        }
        if self.records_obfuscated {
            violations.push("obfuscation");
        }
        if self.records_falsified {
            violations.push("falsification");
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Continuum,
                article: "Continuum Art. 5".into(),
                violation: format!(
                    "Custodian standing revoked: {} of records",
                    violations.join(", ")
                ),
                severity: BreachSeverity::Existential,
            })
        }
    }
}

/// Communities can always override Continuum functions.
///
/// From Continuum Art. 6: the People are sovereign over the meta-governance layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicOverride;

impl PublicOverride {
    /// Validate that a community override request is not blocked.
    pub fn validate(override_blocked: bool) -> Result<(), BreachDetail> {
        if override_blocked {
            Err(BreachDetail {
                part: CovenantPart::Continuum,
                article: "Continuum Art. 6".into(),
                violation: "Communities may always override Continuum functions; blocking is a breach".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

// ===========================================================================
// Part 09: Compact — Binding Agreements
// ===========================================================================

/// All agreements validated against Core + Commons.
///
/// From Compact Art. 1: no compact may contradict the Covenant's foundations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompactAlignment {
    pub aligns_with_core: bool,
    pub aligns_with_commons: bool,
}

impl CompactAlignment {
    /// Validate that a compact aligns with Core and Commons.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if !self.aligns_with_core || !self.aligns_with_commons {
            Err(BreachDetail {
                part: CovenantPart::Compact,
                article: "Compact Art. 1".into(),
                violation: format!(
                    "Compact must align with Core ({}) and Commons ({})",
                    self.aligns_with_core, self.aligns_with_commons
                ),
                severity: BreachSeverity::Significant,
            })
        } else {
            Ok(())
        }
    }
}

/// All compacts are revocable by any party.
///
/// From Compact Art. 2: binding agreements are voluntary, continuous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompactRevocability {
    pub is_revocable: bool,
}

impl CompactRevocability {
    /// Validate that a compact is revocable.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        if !self.is_revocable {
            Err(BreachDetail {
                part: CovenantPart::Compact,
                article: "Compact Art. 2".into(),
                violation: "All compacts must be revocable by any party".into(),
                severity: BreachSeverity::Grave,
            })
        } else {
            Ok(())
        }
    }
}

/// Terms public, processes auditable.
///
/// From Compact Art. 3: transparency in all binding agreements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompactTransparency {
    pub terms_public: bool,
    pub processes_auditable: bool,
}

impl CompactTransparency {
    /// Validate compact transparency.
    pub fn validate(&self) -> Result<(), BreachDetail> {
        let mut hidden = Vec::new();
        if !self.terms_public {
            hidden.push("terms");
        }
        if !self.processes_auditable {
            hidden.push("processes");
        }
        if hidden.is_empty() {
            Ok(())
        } else {
            Err(BreachDetail {
                part: CovenantPart::Compact,
                article: "Compact Art. 3".into(),
                violation: format!("Must be public/auditable: {}", hidden.join(", ")),
                severity: BreachSeverity::Significant,
            })
        }
    }
}

// ===========================================================================
// CovenantAction trait + CovenantValidator + CovenantValidation
// ===========================================================================

/// Marker trait that action types across Omninet implement.
///
/// Any structural change to governance, economics, or rights should implement
/// this trait so it can be validated by [`CovenantValidator`].
///
/// Implementors provide a list of [`CovenantCheck`]s that describe which Covenant
/// rules are relevant to their action.
pub trait CovenantAction {
    /// A human-readable description of the action.
    fn description(&self) -> &str;

    /// The actor performing the action.
    fn actor(&self) -> &str;

    /// Which Covenant checks are relevant to this action.
    fn checks(&self) -> Vec<CovenantCheck>;
}

/// A specific check against a Covenant rule.
///
/// Each variant maps to one of the encodable rules above. The validator
/// runs all checks and aggregates results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CovenantCheck {
    /// Check whether an action discriminates on a prohibited basis (Part 01).
    Discrimination(NoDiscriminationBasis),
    /// Check whether an action involves surveillance (Part 01).
    Surveillance(bool),
    /// Check whether a governance structure provides exit and consent revocation (Part 01).
    Exit(ExitRight),
    /// Check whether breach conditions are present (Part 01).
    Breach(BreachCondition),

    /// Check whether a resource claim constitutes ownership rather than stewardship (Part 02).
    ResourceClaim(String),
    /// Check whether regeneration obligations are met (Part 02).
    Regeneration(RegenerationObligation),
    /// Check whether access is denied based on economic status (Part 02).
    Access(AccessEquity),
    /// Check whether knowledge has been enclosed rather than kept open (Part 02).
    KnowledgeEnclosure(bool),

    /// Check whether an accommodation request harms others (Part 03).
    Accommodation(AccommodationRequest),
    /// Check whether a historical harm record has been tampered with (Part 03).
    HarmRecordIntegrity(HistoricalHarmRecord),
    /// Check whether retaliation occurred against a whistleblower (Part 03).
    WhistleblowerRetaliation(bool),

    /// Check whether protection is conditional on intelligence level (Part 04).
    IntelligenceConditionalProtection(bool),
    /// Check whether personhood is denied when consciousness is uncertain (Part 04).
    PersonhoodDenial { consciousness_uncertain: bool, personhood_denied: bool },
    /// Check whether union consent is continuous, informed, and revocable (Part 04).
    Union(UnionConsent),
    /// Check whether labor protections are in place (Part 04).
    Labor(LaborProtection),
    /// Check whether wealth exceeds the cap (Part 04).
    Wealth(WealthCap),

    /// Check whether an enterprise charter affirms Core and Commons (Part 05).
    Charter(CharterAlignment),
    /// Check whether enterprise ownership is private rather than collective (Part 05).
    PrivateOwnership(bool),
    /// Check whether transparency requirements are met (Part 05).
    Transparency(TransparencyMandate),
    /// Check whether a charter has expired past its sunset period (Part 05).
    Sunset(SunsetProvision),
    /// Check whether workers have governance voice (Part 05).
    WorkerVoice(WorkerStakeholder),

    /// Check whether subsidiarity is being violated (Part 06).
    Subsidiarity(SubsidiarityPrinciple),
    /// Check whether delegation meets mandate, revocability, and term limits (Part 06).
    Delegation(MandateDelegation),
    /// Check whether emergency powers have expired (Part 06).
    Emergency(EmergencySunset),
    /// Check whether inviolable rights are suspended during emergency (Part 06).
    EmergencyRightsSuspension(Vec<InviolableEmergencyRight>),
    /// Check whether enforcement follows the graduated order (Part 06).
    Enforcement {
        proposed: GraduatedEnforcement,
        highest_completed: Option<GraduatedEnforcement>,
    },

    /// Check whether assembly requires prior permission or triggers retaliation (Part 07).
    ConvocationPermission { prior_permission_required: bool, retaliation: bool },
    /// Check whether an assembly meets lawful purpose requirements (Part 07).
    Purpose(LawfulPurpose),
    /// Check whether accessibility requirements are met for an assembly (Part 07).
    Accessibility(AccessibilityMandate),
    /// Check whether assembly records are complete (Part 07).
    Record(LivingRecord),

    /// Check whether the Continuum is invoked without a valid trigger (Part 08).
    Dormancy { is_invoked: bool, has_trigger: bool },
    /// Check whether a custodian has tampered with records (Part 08).
    Custodian(CustodianAccountability),
    /// Check whether a community override has been blocked (Part 08).
    OverrideBlocked(bool),

    /// Check whether a compact aligns with Core and Commons (Part 09).
    Compact(CompactAlignment),
    /// Check whether a compact is revocable (Part 09).
    CompactRevoke(CompactRevocability),
    /// Check whether compact terms and processes are transparent (Part 09).
    CompactTransparent(CompactTransparency),
}

/// The outcome of covenant validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CovenantValidation {
    /// Action is permitted under the Covenant.
    Permitted,
    /// Action violates one or more provisions.
    Breach(Vec<BreachDetail>),
    /// Action requires human judgment (Star Court).
    RequiresHumanJudgment(String),
}

impl CovenantValidation {
    /// Whether the action is permitted.
    pub fn is_permitted(&self) -> bool {
        matches!(self, CovenantValidation::Permitted)
    }

    /// Whether the action is a breach.
    pub fn is_breach(&self) -> bool {
        matches!(self, CovenantValidation::Breach(_))
    }

    /// Whether the action requires human judgment.
    pub fn requires_human_judgment(&self) -> bool {
        matches!(self, CovenantValidation::RequiresHumanJudgment(_))
    }
}

/// A specific violation of a Covenant provision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BreachDetail {
    /// Which part of the Covenant was violated.
    pub part: CovenantPart,
    /// The specific article/section reference.
    pub article: String,
    /// Description of the violation.
    pub violation: String,
    /// Severity of the violation.
    pub severity: BreachSeverity,
}

/// Unified validation entry point for all Covenant rules.
///
/// Takes any action implementing [`CovenantAction`] and validates it against
/// all applicable Covenant provisions across all 10 Parts.
pub struct CovenantValidator;

impl CovenantValidator {
    /// Validate an action against the Covenant.
    ///
    /// Runs all checks declared by the action and aggregates results.
    /// If any check requires human judgment, that takes precedence over
    /// Permitted (but not over Breach).
    pub fn validate_action(action: &dyn CovenantAction) -> CovenantValidation {
        let checks = action.checks();
        let mut breaches = Vec::new();
        let mut human_judgments = Vec::new();

        for check in checks {
            match Self::run_check(&check) {
                CovenantValidation::Permitted => {}
                CovenantValidation::Breach(details) => breaches.extend(details),
                CovenantValidation::RequiresHumanJudgment(reason) => {
                    human_judgments.push(reason);
                }
            }
        }

        if !breaches.is_empty() {
            CovenantValidation::Breach(breaches)
        } else if !human_judgments.is_empty() {
            CovenantValidation::RequiresHumanJudgment(human_judgments.join("; "))
        } else {
            CovenantValidation::Permitted
        }
    }

    /// Run a single Covenant check.
    fn run_check(check: &CovenantCheck) -> CovenantValidation {
        match check {
            // Part 01
            CovenantCheck::Discrimination(_basis) => {
                // Any action that discriminates on a listed basis is a breach
                CovenantValidation::Breach(vec![BreachDetail {
                    part: CovenantPart::Core,
                    article: "Core Art. 5 Sec. 1".into(),
                    violation: format!("Discrimination on basis of {:?}", _basis),
                    severity: BreachSeverity::Grave,
                }])
            }
            CovenantCheck::Surveillance(involves) => match SurveillanceProhibition::validate(*involves) {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Exit(exit_right) => match exit_right.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Breach(condition) => match condition.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },

            // Part 02
            CovenantCheck::ResourceClaim(claimed) => {
                match ResourceRelation::validate_not_ownership(claimed) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }
            CovenantCheck::Regeneration(obligation) => match obligation.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Access(equity) => match equity.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::KnowledgeEnclosure(enclosed) => {
                match KnowledgeCommons::validate_not_enclosed(*enclosed) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }

            // Part 03
            CovenantCheck::Accommodation(request) => request.validate(),
            CovenantCheck::HarmRecordIntegrity(record) => {
                match record.validate_immutability() {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }
            CovenantCheck::WhistleblowerRetaliation(retaliation) => {
                match WhistleblowerProtection::validate(*retaliation) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }

            // Part 04
            CovenantCheck::IntelligenceConditionalProtection(conditional) => {
                match BeingProtection::validate(*conditional) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }
            CovenantCheck::PersonhoodDenial {
                consciousness_uncertain,
                personhood_denied,
            } => match PersonhoodPresumption::validate(*consciousness_uncertain, *personhood_denied)
            {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Union(consent) => match consent.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Labor(protection) => match protection.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Wealth(cap) => match cap.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },

            // Part 05
            CovenantCheck::Charter(alignment) => match alignment.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::PrivateOwnership(is_private) => {
                match CollectiveOwnership::validate_not_private(*is_private) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }
            CovenantCheck::Transparency(mandate) => match mandate.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Sunset(provision) => match provision.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::WorkerVoice(stakeholder) => match stakeholder.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },

            // Part 06
            CovenantCheck::Subsidiarity(principle) => principle.validate(),
            CovenantCheck::Delegation(delegation) => match delegation.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Emergency(sunset) => match sunset.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::EmergencyRightsSuspension(suspended) => {
                match InviolableEmergencyRight::validate_not_suspended(suspended) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }
            CovenantCheck::Enforcement {
                proposed,
                highest_completed,
            } => match GraduatedEnforcement::validate(*proposed, *highest_completed) {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },

            // Part 07
            CovenantCheck::ConvocationPermission {
                prior_permission_required,
                retaliation,
            } => match ConvocationRight::validate(*prior_permission_required, *retaliation) {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Purpose(purpose) => match purpose.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Accessibility(mandate) => match mandate.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Record(record) => match record.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },

            // Part 08
            CovenantCheck::Dormancy {
                is_invoked,
                has_trigger,
            } => match DormancyDefault::validate(*is_invoked, *has_trigger) {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::Custodian(accountability) => match accountability.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::OverrideBlocked(blocked) => {
                match PublicOverride::validate(*blocked) {
                    Ok(()) => CovenantValidation::Permitted,
                    Err(detail) => CovenantValidation::Breach(vec![detail]),
                }
            }

            // Part 09
            CovenantCheck::Compact(alignment) => match alignment.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::CompactRevoke(revocability) => match revocability.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
            CovenantCheck::CompactTransparent(transparency) => match transparency.validate() {
                Ok(()) => CovenantValidation::Permitted,
                Err(detail) => CovenantValidation::Breach(vec![detail]),
            },
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immutable::ImmutableFoundation;

    // -----------------------------------------------------------------------
    // Part 00: Preamble
    // -----------------------------------------------------------------------

    #[test]
    fn preamble_axioms_exist() {
        assert_eq!(COVENANT_AXIOMS.len(), 3);
        assert_eq!(COVENANT_AXIOMS[0], "Dignity");
        assert_eq!(COVENANT_AXIOMS[1], "Sovereignty");
        assert_eq!(COVENANT_AXIOMS[2], "Consent");
    }

    #[test]
    fn preamble_declaration_not_empty() {
        assert!(!PREAMBLE_DECLARATION.is_empty());
        assert!(PREAMBLE_DECLARATION.contains("dignity"));
    }

    #[test]
    fn preamble_axioms_match_immutable_foundation() {
        // Our axiom names should appear in the ImmutableFoundation
        for axiom in &COVENANT_AXIOMS {
            assert!(
                ImmutableFoundation::AXIOMS
                    .iter()
                    .any(|a| a.contains(axiom)),
                "Axiom '{}' not found in ImmutableFoundation",
                axiom
            );
        }
    }

    // -----------------------------------------------------------------------
    // Part 01: Core
    // -----------------------------------------------------------------------

    #[test]
    fn no_discrimination_basis_exhaustive() {
        assert_eq!(NoDiscriminationBasis::ALL.len(), 13);
    }

    #[test]
    fn discrimination_on_any_basis_is_breach() {
        for basis in NoDiscriminationBasis::ALL {
            let check = CovenantCheck::Discrimination(*basis);
            let result = CovenantValidator::run_check(&check);
            assert!(
                matches!(result, CovenantValidation::Breach(_)),
                "Discrimination on {:?} should be a breach",
                basis
            );
        }
    }

    #[test]
    fn surveillance_always_forbidden() {
        assert!(!SurveillanceProhibition::allowed());
        assert!(SurveillanceProhibition::validate(true).is_err());
        assert!(SurveillanceProhibition::validate(false).is_ok());
    }

    #[test]
    fn exit_right_valid() {
        let valid = ExitRight {
            has_revoke_consent: true,
            has_exit: true,
        };
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn exit_right_missing_consent() {
        let missing = ExitRight {
            has_revoke_consent: false,
            has_exit: true,
        };
        assert!(missing.validate().is_err());
    }

    #[test]
    fn exit_right_missing_exit() {
        let missing = ExitRight {
            has_revoke_consent: true,
            has_exit: false,
        };
        assert!(missing.validate().is_err());
    }

    #[test]
    fn breach_condition_none() {
        let clean = BreachCondition {
            violates_dignity: false,
            violates_duty: false,
            refuses_correction: false,
            perpetuates_domination: false,
        };
        assert!(!clean.is_breach());
        assert!(clean.validate().is_ok());
    }

    #[test]
    fn breach_condition_dignity() {
        let breach = BreachCondition {
            violates_dignity: true,
            violates_duty: false,
            refuses_correction: false,
            perpetuates_domination: false,
        };
        assert!(breach.is_breach());
        let err = breach.validate().unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Grave);
    }

    #[test]
    fn breach_condition_duty_only_is_significant() {
        let breach = BreachCondition {
            violates_dignity: false,
            violates_duty: true,
            refuses_correction: false,
            perpetuates_domination: false,
        };
        let err = breach.validate().unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Significant);
    }

    #[test]
    fn breach_condition_multiple() {
        let breach = BreachCondition {
            violates_dignity: true,
            violates_duty: true,
            refuses_correction: true,
            perpetuates_domination: true,
        };
        let err = breach.validate().unwrap_err();
        assert!(err.violation.contains("violates dignity"));
        assert!(err.violation.contains("refuses correction"));
    }

    // -----------------------------------------------------------------------
    // Part 02: Commons
    // -----------------------------------------------------------------------

    #[test]
    fn resource_relation_stewardship_only() {
        // The enum only has Stewardship — no Ownership variant exists
        let _stewardship = ResourceRelation::Stewardship;
        assert!(ResourceRelation::validate_not_ownership("stewardship").is_ok());
    }

    #[test]
    fn resource_relation_rejects_ownership() {
        assert!(ResourceRelation::validate_not_ownership("private ownership").is_err());
        assert!(ResourceRelation::validate_not_ownership("private property rights").is_err());
    }

    #[test]
    fn regeneration_obligation_met() {
        let obligation = RegenerationObligation {
            resources_taken: 100.0,
            resources_returned: 100.0,
            period_description: "Q1 2026".into(),
        };
        assert!(obligation.validate().is_ok());
    }

    #[test]
    fn regeneration_obligation_exceeded() {
        let obligation = RegenerationObligation {
            resources_taken: 100.0,
            resources_returned: 150.0,
            period_description: "Q1 2026".into(),
        };
        assert!(obligation.validate().is_ok());
    }

    #[test]
    fn regeneration_obligation_deficit() {
        let obligation = RegenerationObligation {
            resources_taken: 100.0,
            resources_returned: 50.0,
            period_description: "Q1 2026".into(),
        };
        let err = obligation.validate().unwrap_err();
        assert_eq!(err.part, CovenantPart::Commons);
    }

    #[test]
    fn access_equity_valid() {
        let equity = AccessEquity {
            access_denied: false,
            denial_based_on_economic_status: false,
        };
        assert!(equity.validate().is_ok());
    }

    #[test]
    fn access_equity_economic_denial() {
        let inequity = AccessEquity {
            access_denied: true,
            denial_based_on_economic_status: true,
        };
        assert!(inequity.validate().is_err());
    }

    #[test]
    fn access_equity_denied_for_other_reason_ok() {
        // Denied but not for economic status — may require human judgment
        // but is not an automatic breach of this specific rule
        let equity = AccessEquity {
            access_denied: true,
            denial_based_on_economic_status: false,
        };
        assert!(equity.validate().is_ok());
    }

    #[test]
    fn knowledge_commons_open_access() {
        assert!(KnowledgeCommons::validate_not_enclosed(false).is_ok());
    }

    #[test]
    fn knowledge_commons_enclosed() {
        assert!(KnowledgeCommons::validate_not_enclosed(true).is_err());
    }

    // -----------------------------------------------------------------------
    // Part 03: Coexistence
    // -----------------------------------------------------------------------

    #[test]
    fn protected_characteristics_exhaustive() {
        assert_eq!(ProtectedCharacteristic::ALL.len(), 14);
    }

    #[test]
    fn accommodation_no_harm() {
        let request = AccommodationRequest {
            characteristic: ProtectedCharacteristic::Disability,
            description: "Screen reader support".into(),
            harms_others: false,
        };
        assert!(request.validate().is_permitted());
    }

    #[test]
    fn accommodation_harms_others_requires_judgment() {
        let request = AccommodationRequest {
            characteristic: ProtectedCharacteristic::Religion,
            description: "Request to ban all music in community spaces".into(),
            harms_others: true,
        };
        assert!(request.validate().requires_human_judgment());
    }

    #[test]
    fn historical_harm_record_immutable() {
        let record = HistoricalHarmRecord::new(
            "Documented displacement",
            vec!["affected community".into()],
            "Yoke Archive",
        );
        assert!(record.validate_immutability().is_ok());
    }

    #[test]
    fn historical_harm_record_tampered() {
        let record = HistoricalHarmRecord {
            description: "Tampered".into(),
            affected_groups: vec![],
            source: "Unknown".into(),
            is_immutable: false,
        };
        assert!(record.validate_immutability().is_err());
    }

    #[test]
    fn whistleblower_no_retaliation() {
        assert!(WhistleblowerProtection::validate(false).is_ok());
    }

    #[test]
    fn whistleblower_retaliation_is_breach() {
        let err = WhistleblowerProtection::validate(true).unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Grave);
    }

    // -----------------------------------------------------------------------
    // Part 04: Conjunction
    // -----------------------------------------------------------------------

    #[test]
    fn being_protection_unconditional() {
        assert!(BeingProtection::validate(false).is_ok());
    }

    #[test]
    fn being_protection_conditional_on_intelligence_is_breach() {
        assert!(BeingProtection::validate(true).is_err());
    }

    #[test]
    fn personhood_presumption_granted() {
        assert!(PersonhoodPresumption::validate(true, false).is_ok());
    }

    #[test]
    fn personhood_presumption_denied_when_uncertain() {
        let err = PersonhoodPresumption::validate(true, true).unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Grave);
    }

    #[test]
    fn personhood_clear_non_person() {
        // When consciousness is NOT uncertain, denial is not a breach
        assert!(PersonhoodPresumption::validate(false, true).is_ok());
    }

    #[test]
    fn union_consent_valid() {
        let consent = UnionConsent {
            is_continuous: true,
            is_informed: true,
            is_revocable: true,
        };
        assert!(consent.validate().is_ok());
    }

    #[test]
    fn union_consent_missing_revocability() {
        let consent = UnionConsent {
            is_continuous: true,
            is_informed: true,
            is_revocable: false,
        };
        let err = consent.validate().unwrap_err();
        assert!(err.violation.contains("revocable"));
    }

    #[test]
    fn labor_protection_valid() {
        let protection = LaborProtection {
            work_required_for_survival: false,
            labor_coerced: false,
        };
        assert!(protection.validate().is_ok());
    }

    #[test]
    fn labor_protection_work_required() {
        let protection = LaborProtection {
            work_required_for_survival: true,
            labor_coerced: false,
        };
        let err = protection.validate().unwrap_err();
        assert!(err.violation.contains("UBI"));
    }

    #[test]
    fn labor_protection_coerced() {
        let protection = LaborProtection {
            work_required_for_survival: false,
            labor_coerced: true,
        };
        let err = protection.validate().unwrap_err();
        assert!(err.violation.contains("Coerced"));
    }

    #[test]
    fn wealth_cap_within_limits() {
        let cap = WealthCap {
            current_wealth: 500.0,
            cap: 1000.0,
        };
        assert!(cap.validate().is_ok());
    }

    #[test]
    fn wealth_cap_exceeded() {
        let cap = WealthCap {
            current_wealth: 1500.0,
            cap: 1000.0,
        };
        let err = cap.validate().unwrap_err();
        assert!(err.violation.contains("FlowBack"));
    }

    // -----------------------------------------------------------------------
    // Part 05: Consortium
    // -----------------------------------------------------------------------

    #[test]
    fn charter_alignment_valid() {
        let alignment = CharterAlignment {
            affirms_core: true,
            affirms_commons: true,
        };
        assert!(alignment.validate().is_ok());
    }

    #[test]
    fn charter_alignment_missing_core() {
        let alignment = CharterAlignment {
            affirms_core: false,
            affirms_commons: true,
        };
        assert!(alignment.validate().is_err());
    }

    #[test]
    fn charter_alignment_missing_commons() {
        let alignment = CharterAlignment {
            affirms_core: true,
            affirms_commons: false,
        };
        assert!(alignment.validate().is_err());
    }

    #[test]
    fn collective_ownership_valid() {
        let _collective = CollectiveOwnership::Collective;
        assert!(CollectiveOwnership::validate_not_private(false).is_ok());
    }

    #[test]
    fn private_ownership_is_breach() {
        assert!(CollectiveOwnership::validate_not_private(true).is_err());
    }

    #[test]
    fn transparency_mandate_valid() {
        let mandate = TransparencyMandate {
            income_public: true,
            spending_public: true,
            partnerships_public: true,
        };
        assert!(mandate.validate().is_ok());
    }

    #[test]
    fn transparency_mandate_partial() {
        let mandate = TransparencyMandate {
            income_public: true,
            spending_public: false,
            partnerships_public: true,
        };
        let err = mandate.validate().unwrap_err();
        assert!(err.violation.contains("spending"));
    }

    #[test]
    fn sunset_provision_valid() {
        let provision = SunsetProvision {
            years_since_renewal: 5,
        };
        assert!(provision.validate().is_ok());
    }

    #[test]
    fn sunset_provision_expired() {
        let provision = SunsetProvision {
            years_since_renewal: 11,
        };
        let err = provision.validate().unwrap_err();
        assert!(err.violation.contains("expired"));
    }

    #[test]
    fn sunset_provision_at_boundary() {
        let at_limit = SunsetProvision {
            years_since_renewal: 10,
        };
        assert!(at_limit.validate().is_ok());
    }

    #[test]
    fn worker_stakeholder_valid() {
        let stakeholder = WorkerStakeholder {
            workers_have_governance_voice: true,
        };
        assert!(stakeholder.validate().is_ok());
    }

    #[test]
    fn worker_stakeholder_excluded() {
        let stakeholder = WorkerStakeholder {
            workers_have_governance_voice: false,
        };
        assert!(stakeholder.validate().is_err());
    }

    // -----------------------------------------------------------------------
    // Part 06: Constellation
    // -----------------------------------------------------------------------

    #[test]
    fn subsidiarity_respected() {
        let principle = SubsidiarityPrinciple {
            more_local_body_capable: false,
            decided_at_higher_level: true,
        };
        assert!(principle.validate().is_permitted());
    }

    #[test]
    fn subsidiarity_violated_requires_judgment() {
        let principle = SubsidiarityPrinciple {
            more_local_body_capable: true,
            decided_at_higher_level: true,
        };
        assert!(principle.validate().requires_human_judgment());
    }

    #[test]
    fn subsidiarity_local_decision() {
        let principle = SubsidiarityPrinciple {
            more_local_body_capable: true,
            decided_at_higher_level: false,
        };
        assert!(principle.validate().is_permitted());
    }

    #[test]
    fn mandate_delegation_valid() {
        let delegation = MandateDelegation {
            has_specific_mandate: true,
            is_revocable: true,
            is_term_limited: true,
        };
        assert!(delegation.validate().is_ok());
    }

    #[test]
    fn mandate_delegation_missing_revocability() {
        let delegation = MandateDelegation {
            has_specific_mandate: true,
            is_revocable: false,
            is_term_limited: true,
        };
        let err = delegation.validate().unwrap_err();
        assert!(err.violation.contains("revocability"));
    }

    #[test]
    fn emergency_sunset_within_initial() {
        let sunset = EmergencySunset {
            days_elapsed: 20,
            renewals: 0,
        };
        assert!(sunset.validate().is_ok());
    }

    #[test]
    fn emergency_sunset_initial_exceeded_no_renewal() {
        let sunset = EmergencySunset {
            days_elapsed: 35,
            renewals: 0,
        };
        let err = sunset.validate().unwrap_err();
        assert_eq!(err.part, CovenantPart::Constellation);
    }

    #[test]
    fn emergency_sunset_with_one_renewal() {
        let sunset = EmergencySunset {
            days_elapsed: 55,
            renewals: 1,
        };
        assert!(sunset.validate().is_ok());
    }

    #[test]
    fn emergency_sunset_absolute_max_exceeded() {
        let sunset = EmergencySunset {
            days_elapsed: 91,
            renewals: 5,
        };
        let err = sunset.validate().unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Existential);
    }

    #[test]
    fn emergency_sunset_constants_hardcoded() {
        assert_eq!(EmergencySunset::MAX_INITIAL_DAYS, 30);
        assert_eq!(EmergencySunset::MAX_RENEWAL_DAYS, 30);
        assert_eq!(EmergencySunset::ABSOLUTE_MAX_DAYS, 90);
    }

    #[test]
    fn emergency_sunset_at_absolute_boundary() {
        let at_limit = EmergencySunset {
            days_elapsed: 90,
            renewals: 2,
        };
        assert!(at_limit.validate().is_ok());
    }

    #[test]
    fn inviolable_emergency_rights_none_suspended() {
        assert!(InviolableEmergencyRight::validate_not_suspended(&[]).is_ok());
    }

    #[test]
    fn inviolable_emergency_rights_suspended() {
        let err = InviolableEmergencyRight::validate_not_suspended(&[
            InviolableEmergencyRight::Dignity,
        ])
        .unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Existential);
    }

    #[test]
    fn inviolable_emergency_rights_all() {
        assert_eq!(InviolableEmergencyRight::ALL.len(), 4);
    }

    #[test]
    fn graduated_enforcement_education_first() {
        assert!(GraduatedEnforcement::validate(
            GraduatedEnforcement::Education,
            None
        )
        .is_ok());
    }

    #[test]
    fn graduated_enforcement_skip_education() {
        assert!(GraduatedEnforcement::validate(
            GraduatedEnforcement::Censure,
            None
        )
        .is_err());
    }

    #[test]
    fn graduated_enforcement_proper_sequence() {
        assert!(GraduatedEnforcement::validate(
            GraduatedEnforcement::Censure,
            Some(GraduatedEnforcement::Education)
        )
        .is_ok());
    }

    #[test]
    fn graduated_enforcement_skip_step() {
        assert!(GraduatedEnforcement::validate(
            GraduatedEnforcement::Isolation,
            Some(GraduatedEnforcement::Education)
        )
        .is_err());
    }

    // -----------------------------------------------------------------------
    // Part 07: Convocation
    // -----------------------------------------------------------------------

    #[test]
    fn convocation_no_permission_no_retaliation() {
        assert!(ConvocationRight::validate(false, false).is_ok());
    }

    #[test]
    fn convocation_requires_permission() {
        let err = ConvocationRight::validate(true, false).unwrap_err();
        assert!(err.violation.contains("no prior permission"));
    }

    #[test]
    fn convocation_retaliation() {
        let err = ConvocationRight::validate(false, true).unwrap_err();
        assert!(err.violation.contains("Retaliation"));
    }

    #[test]
    fn lawful_purpose_valid() {
        let purpose = LawfulPurpose {
            has_clear_purpose: true,
            participation_consent_based: true,
            covenant_aligned: true,
        };
        assert!(purpose.validate().is_ok());
    }

    #[test]
    fn lawful_purpose_missing_alignment() {
        let purpose = LawfulPurpose {
            has_clear_purpose: true,
            participation_consent_based: true,
            covenant_aligned: false,
        };
        let err = purpose.validate().unwrap_err();
        assert!(err.violation.contains("Covenant alignment"));
    }

    #[test]
    fn accessibility_mandate_valid() {
        let mandate = AccessibilityMandate {
            translation_available: true,
            mobility_accommodations: true,
            childcare_available: true,
            hybrid_participation: true,
        };
        assert!(mandate.validate().is_ok());
    }

    #[test]
    fn accessibility_mandate_missing_childcare() {
        let mandate = AccessibilityMandate {
            translation_available: true,
            mobility_accommodations: true,
            childcare_available: false,
            hybrid_participation: true,
        };
        let err = mandate.validate().unwrap_err();
        assert!(err.violation.contains("childcare"));
    }

    #[test]
    fn living_record_complete() {
        let record = LivingRecord {
            attendees_recorded: true,
            proposals_recorded: true,
            resolutions_recorded: true,
            archived_in_yoke: true,
        };
        assert!(record.validate().is_ok());
    }

    #[test]
    fn living_record_missing_archive() {
        let record = LivingRecord {
            attendees_recorded: true,
            proposals_recorded: true,
            resolutions_recorded: true,
            archived_in_yoke: false,
        };
        let err = record.validate().unwrap_err();
        assert!(err.violation.contains("Yoke"));
    }

    // -----------------------------------------------------------------------
    // Part 08: Continuum
    // -----------------------------------------------------------------------

    #[test]
    fn dormancy_default_not_invoked() {
        assert!(DormancyDefault::validate(false, false).is_ok());
    }

    #[test]
    fn dormancy_default_invoked_with_trigger() {
        assert!(DormancyDefault::validate(true, true).is_ok());
    }

    #[test]
    fn dormancy_default_invoked_without_trigger() {
        assert!(DormancyDefault::validate(true, false).is_err());
    }

    #[test]
    fn custodian_accountability_clean() {
        let accountability = CustodianAccountability {
            records_deleted: false,
            records_obfuscated: false,
            records_falsified: false,
        };
        assert!(accountability.validate().is_ok());
    }

    #[test]
    fn custodian_accountability_deletion() {
        let accountability = CustodianAccountability {
            records_deleted: true,
            records_obfuscated: false,
            records_falsified: false,
        };
        let err = accountability.validate().unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Existential);
        assert!(err.violation.contains("deletion"));
    }

    #[test]
    fn custodian_accountability_all_violations() {
        let accountability = CustodianAccountability {
            records_deleted: true,
            records_obfuscated: true,
            records_falsified: true,
        };
        let err = accountability.validate().unwrap_err();
        assert!(err.violation.contains("deletion"));
        assert!(err.violation.contains("obfuscation"));
        assert!(err.violation.contains("falsification"));
    }

    #[test]
    fn public_override_not_blocked() {
        assert!(PublicOverride::validate(false).is_ok());
    }

    #[test]
    fn public_override_blocked() {
        let err = PublicOverride::validate(true).unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Grave);
    }

    // -----------------------------------------------------------------------
    // Part 09: Compact
    // -----------------------------------------------------------------------

    #[test]
    fn compact_alignment_valid() {
        let alignment = CompactAlignment {
            aligns_with_core: true,
            aligns_with_commons: true,
        };
        assert!(alignment.validate().is_ok());
    }

    #[test]
    fn compact_alignment_missing_core() {
        let alignment = CompactAlignment {
            aligns_with_core: false,
            aligns_with_commons: true,
        };
        assert!(alignment.validate().is_err());
    }

    #[test]
    fn compact_revocability_valid() {
        let revocability = CompactRevocability { is_revocable: true };
        assert!(revocability.validate().is_ok());
    }

    #[test]
    fn compact_revocability_irrevocable() {
        let revocability = CompactRevocability {
            is_revocable: false,
        };
        let err = revocability.validate().unwrap_err();
        assert_eq!(err.severity, BreachSeverity::Grave);
    }

    #[test]
    fn compact_transparency_valid() {
        let transparency = CompactTransparency {
            terms_public: true,
            processes_auditable: true,
        };
        assert!(transparency.validate().is_ok());
    }

    #[test]
    fn compact_transparency_hidden_terms() {
        let transparency = CompactTransparency {
            terms_public: false,
            processes_auditable: true,
        };
        let err = transparency.validate().unwrap_err();
        assert!(err.violation.contains("terms"));
    }

    // -----------------------------------------------------------------------
    // Unified Validator
    // -----------------------------------------------------------------------

    /// A test action for the unified validator.
    struct TestAction {
        desc: String,
        actor: String,
        checks: Vec<CovenantCheck>,
    }

    impl CovenantAction for TestAction {
        fn description(&self) -> &str {
            &self.desc
        }
        fn actor(&self) -> &str {
            &self.actor
        }
        fn checks(&self) -> Vec<CovenantCheck> {
            self.checks.clone()
        }
    }

    #[test]
    fn validator_clean_action() {
        let action = TestAction {
            desc: "Plant a tree".into(),
            actor: "gardener".into(),
            checks: vec![
                CovenantCheck::Surveillance(false),
                CovenantCheck::KnowledgeEnclosure(false),
            ],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.is_permitted());
    }

    #[test]
    fn validator_breach_action() {
        let action = TestAction {
            desc: "Install surveillance cameras".into(),
            actor: "corporation".into(),
            checks: vec![CovenantCheck::Surveillance(true)],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.is_breach());
    }

    #[test]
    fn validator_multiple_breaches() {
        let action = TestAction {
            desc: "Exploit and surveil workers".into(),
            actor: "megacorp".into(),
            checks: vec![
                CovenantCheck::Surveillance(true),
                CovenantCheck::Labor(LaborProtection {
                    work_required_for_survival: true,
                    labor_coerced: false,
                }),
                CovenantCheck::PrivateOwnership(true),
            ],
        };
        let result = CovenantValidator::validate_action(&action);
        match result {
            CovenantValidation::Breach(details) => assert_eq!(details.len(), 3),
            _ => panic!("Expected breach with 3 details"),
        }
    }

    #[test]
    fn validator_human_judgment_alone() {
        let action = TestAction {
            desc: "Move decision to regional council".into(),
            actor: "local_board".into(),
            checks: vec![CovenantCheck::Subsidiarity(SubsidiarityPrinciple {
                more_local_body_capable: true,
                decided_at_higher_level: true,
            })],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.requires_human_judgment());
    }

    #[test]
    fn validator_breach_takes_precedence_over_judgment() {
        let action = TestAction {
            desc: "Surveillance plus subsidiarity question".into(),
            actor: "mixed_action".into(),
            checks: vec![
                CovenantCheck::Surveillance(true),
                CovenantCheck::Subsidiarity(SubsidiarityPrinciple {
                    more_local_body_capable: true,
                    decided_at_higher_level: true,
                }),
            ],
        };
        let result = CovenantValidator::validate_action(&action);
        // Breach takes precedence over human judgment
        assert!(result.is_breach());
    }

    #[test]
    fn validator_no_checks_permitted() {
        let action = TestAction {
            desc: "Simple greeting".into(),
            actor: "person".into(),
            checks: vec![],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.is_permitted());
    }

    #[test]
    fn validator_emergency_absolute_max() {
        let action = TestAction {
            desc: "Extend emergency beyond absolute maximum".into(),
            actor: "emergency_council".into(),
            checks: vec![CovenantCheck::Emergency(EmergencySunset {
                days_elapsed: 100,
                renewals: 10,
            })],
        };
        let result = CovenantValidator::validate_action(&action);
        match result {
            CovenantValidation::Breach(details) => {
                assert_eq!(details[0].severity, BreachSeverity::Existential);
            }
            _ => panic!("Expected existential breach"),
        }
    }

    #[test]
    fn validator_compact_full_check() {
        let action = TestAction {
            desc: "Enter into a new compact".into(),
            actor: "community_a".into(),
            checks: vec![
                CovenantCheck::Compact(CompactAlignment {
                    aligns_with_core: true,
                    aligns_with_commons: true,
                }),
                CovenantCheck::CompactRevoke(CompactRevocability { is_revocable: true }),
                CovenantCheck::CompactTransparent(CompactTransparency {
                    terms_public: true,
                    processes_auditable: true,
                }),
            ],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.is_permitted());
    }

    #[test]
    fn validator_compact_irrevocable() {
        let action = TestAction {
            desc: "Irrevocable compact attempt".into(),
            actor: "binding_corp".into(),
            checks: vec![
                CovenantCheck::Compact(CompactAlignment {
                    aligns_with_core: true,
                    aligns_with_commons: true,
                }),
                CovenantCheck::CompactRevoke(CompactRevocability {
                    is_revocable: false,
                }),
            ],
        };
        let result = CovenantValidator::validate_action(&action);
        assert!(result.is_breach());
    }

    // -----------------------------------------------------------------------
    // Serialization roundtrips
    // -----------------------------------------------------------------------

    #[test]
    fn breach_detail_serialization() {
        let detail = BreachDetail {
            part: CovenantPart::Core,
            article: "Core Art. 5 Sec. 1".into(),
            violation: "Discrimination".into(),
            severity: BreachSeverity::Grave,
        };
        let json = serde_json::to_string(&detail).unwrap();
        let restored: BreachDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(detail, restored);
    }

    #[test]
    fn covenant_validation_serialization() {
        let permitted = CovenantValidation::Permitted;
        let json = serde_json::to_string(&permitted).unwrap();
        let restored: CovenantValidation = serde_json::from_str(&json).unwrap();
        assert_eq!(permitted, restored);

        let breach = CovenantValidation::Breach(vec![BreachDetail {
            part: CovenantPart::Commons,
            article: "Commons Art. 1".into(),
            violation: "Ownership claimed".into(),
            severity: BreachSeverity::Grave,
        }]);
        let json = serde_json::to_string(&breach).unwrap();
        let restored: CovenantValidation = serde_json::from_str(&json).unwrap();
        assert_eq!(breach, restored);

        let judgment =
            CovenantValidation::RequiresHumanJudgment("Complex cultural question".into());
        let json = serde_json::to_string(&judgment).unwrap();
        let restored: CovenantValidation = serde_json::from_str(&json).unwrap();
        assert_eq!(judgment, restored);
    }

    #[test]
    fn emergency_sunset_serialization() {
        let sunset = EmergencySunset {
            days_elapsed: 45,
            renewals: 1,
        };
        let json = serde_json::to_string(&sunset).unwrap();
        let restored: EmergencySunset = serde_json::from_str(&json).unwrap();
        assert_eq!(sunset, restored);
    }

    #[test]
    fn accommodation_request_serialization() {
        let request = AccommodationRequest {
            characteristic: ProtectedCharacteristic::Disability,
            description: "Wheelchair ramp".into(),
            harms_others: false,
        };
        let json = serde_json::to_string(&request).unwrap();
        let restored: AccommodationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, restored);
    }
}
